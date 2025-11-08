//! Package version switching functionality

use crate::db::PackageDB;
use crate::error::SwitchError;
use crate::package::installer::create_symlinks;
use crate::{info, warn};
use semver::Version;

/// Switches the active version of a package
///
/// # Arguments
/// - `pkg_name`: package name
/// - `target_version`: version to switch to
/// - `db`: package database instance
/// - `direct`: direct symlink mode
///
/// # Errors
/// Returns `SwitchError` on filesystem or database failures
pub async fn switch_version(
    pkg_name: &str,
    target_version: Version,
    db: &PackageDB,
    direct: bool,
) -> Result<(), SwitchError> {
    let home_dir = dirs::home_dir().ok_or_else(|| {
        SwitchError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME directory not found",
        ))
    })?;

    // Remove old symlinks if current version exists
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

    // Create symlinks for new version
    create_symlinks(&new_pkg_dir, direct)?;

    // Update database
    db.set_current_version(pkg_name, &target_version.to_string())
        .await?;

    info!("package.switcher.switch_success", pkg_name, target_version);

    Ok(())
}
