//! # Package Version Switcher
//!
//! This module provides functionality to switch between different installed
//! versions of a package. It updates symbolic links to point to the files
//! of the desired version and updates the database record for the "current"
//! version.
//!
//! ## Responsibilities
//! - Remove symlinks of the currently active version.
//! - Validate existence of the target version directory.
//! - Create symlinks for the target version.
//! - Update the package database with the new current version.
//!
//! Errors are unified under [`SwitchError`] for consistency.

use crate::db::PackageDB;
use crate::error::SwitchError;
use crate::package::installer::create_symlinks;
use crate::{info, warn};
use semver::Version;

/// Errors that may occur when switching package versions.
// #[derive(Debug)]
// pub enum SwitchError {
//     /// Filesystem or I/O error.
//     Io(std::io::Error),
//     /// Database error from `sqlx`.
//     Db(sqlx::Error),
//     /// Target package directory does not exist.
//     MissingPackageDir(PathBuf),
//     /// Error while parsing or processing `symlist.ron`.
//     Symlist(crate::symlist::SymlistError),
//     /// Requested package version not found in database.
//     PackageNotFound(String, Version),
// }

// impl From<std::io::Error> for SwitchError {
//     fn from(e: std::io::Error) -> Self {
//         SwitchError::Io(e)
//     }
// }
// impl From<sqlx::Error> for SwitchError {
//     fn from(e: sqlx::Error) -> Self {
//         SwitchError::Db(e)
//     }
// }
// impl From<crate::symlist::SymlistError> for SwitchError {
//     fn from(e: crate::symlist::SymlistError) -> Self {
//         SwitchError::Symlist(e)
//     }
// }

/// Switch the active version of a package.
///
/// # Arguments
/// - `pkg_name`: The package name.
/// - `target_version`: The version to switch to.
/// - `db`: Reference to the [`PackageDB`] instance.
///
/// # Workflow
/// 1. Remove symlinks of the current active version (if present).
///    - Ensures only symlinks created by UHPM are removed.
///    - Non-matching symlinks or regular files are skipped safely.
/// 2. Verify that the target package directory exists.
///    - If not, returns [`SwitchError::MissingPackageDir`].
/// 3. Create symlinks for the target version using [`create_symlinks`].
/// 4. Update the package database with the new current version.
///
/// # Errors
/// Returns [`SwitchError`] if:
/// - Filesystem operations (removing files, reading symlinks) fail.
/// - Database operations fail.
/// - `symlist.ron` is missing or invalid.
/// - Target package directory does not exist.
///
/// # Logging
/// - Logs removed or skipped symlinks from the old version.
/// - Logs the switch to the new version when successful.
pub async fn switch_version(
    pkg_name: &str,
    target_version: Version,
    db: &PackageDB,
) -> Result<(), SwitchError> {
    // Get home directory safely
    let home_dir = dirs::home_dir().ok_or_else(|| {
        SwitchError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME directory not found",
        ))
    })?;

    // Remove symlinks from the current version if available
    if let Some(current_package) = db.get_current_package(pkg_name).await? {
        let current_version_str = current_package.version().to_string();
        let current_pkg_dir = home_dir
            .join(".uhpm/packages")
            .join(format!("{}-{}", pkg_name, current_version_str));

        if current_pkg_dir.exists() {
            let symlist_path = current_pkg_dir.join("symlist.ron");
            match crate::symlist::load_symlist(&symlist_path, &current_pkg_dir) {
                Ok(symlinks) => {
                    for (src_abs, dst_abs) in symlinks {
                        if !dst_abs.exists() {
                            continue;
                        }

                        match std::fs::symlink_metadata(&dst_abs) {
                            Ok(meta) if meta.file_type().is_symlink() => {
                                match std::fs::read_link(&dst_abs) {
                                    Ok(link_target) if link_target == src_abs => {
                                        if let Err(e) = std::fs::remove_file(&dst_abs) {
                                            warn!(
                                                "package.switcher.remove_symlink_failed",
                                                dst_abs.display(),
                                                e
                                            );
                                        } else {
                                            info!(
                                                "package.switcher.removed_old_symlink",
                                                dst_abs.display()
                                            );
                                        }
                                    }
                                    Ok(link_target) => {
                                        info!(
                                            "package.switcher.skipping_symlink_wrong_target",
                                            dst_abs.display(),
                                            src_abs.display(),
                                            link_target.display()
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            "package.switcher.read_symlink_failed",
                                            dst_abs.display(),
                                            e
                                        );
                                    }
                                }
                            }
                            Ok(_) => {
                                info!("package.switcher.skipping_not_symlink", dst_abs.display())
                            }
                            Err(e) => {
                                warn!("package.switcher.metadata_failed", dst_abs.display(), e)
                            }
                        }
                    }
                }
                Err(crate::symlist::SymlistError::Io(ref io_err))
                    if io_err.kind() == std::io::ErrorKind::NotFound =>
                {
                    info!("package.switcher.symlist_not_found_cleanup_skip");
                }
                Err(e) => return Err(SwitchError::Symlist(e)),
            }
        } else {
            info!(
                "package.switcher.package_dir_not_found_cleanup_skip",
                current_pkg_dir.display()
            );
        }
    } else {
        info!("package.switcher.no_current_version_cleanup_skip");
    }

    // Verify target package directory exists
    let new_pkg_dir = home_dir
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, target_version));

    if !new_pkg_dir.exists() {
        return Err(SwitchError::MissingPackageDir(new_pkg_dir));
    }

    // Create symlinks for the new version
    create_symlinks(&new_pkg_dir)?;

    // Update database with the new current version
    db.set_current_version(pkg_name, &target_version.to_string())
        .await?;

    info!("package.switcher.switch_success", pkg_name, target_version);

    Ok(())
}
