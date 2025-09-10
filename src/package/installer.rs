//! # Package Installer
//!
//! This module provides functionality for installing `.uhp` packages into the
//! user's `~/.uhpm/packages` directory. It handles unpacking, reading metadata,
//! creating symbolic links, and updating the package database.
//!
//! ## Responsibilities
//! - Unpack `.uhp` archives into a temporary directory.
//! - Parse package metadata (`uhp.ron`).
//! - Move unpacked contents into the package root directory.
//! - Create symbolic links based on `symlist.ron`.
//! - Update the local package database with installed files and current version.
//!
//! Errors are unified under [`InstallError`] for convenience.

use crate::db::PackageDB;
use crate::package::Package;
use crate::symlist;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug,info, warn};

/// Errors that can occur during package installation.
#[derive(Debug)]
pub enum InstallError {
    /// Filesystem or I/O error.
    Io(std::io::Error),
    /// Error parsing package metadata.
    Meta(crate::package::MetaParseError),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

impl From<crate::package::MetaParseError> for InstallError {
    fn from(e: crate::package::MetaParseError) -> Self {
        InstallError::Meta(e)
    }
}

/// Install a package from a `.uhp` archive.
///
/// # Arguments
/// - `pkg_path`: Path to the `.uhp` file.
/// - `db`: Reference to the [`PackageDB`] instance.
///
/// # Workflow
/// 1. Unpack the `.uhp` archive into a temporary directory.
/// 2. Parse the `uhp.ron` metadata file into a [`Package`].
/// 3. Check if the package is already installed:
///    - If the same version exists, installation is skipped.
///    - If a different version exists, the new version replaces it.
/// 4. Move unpacked contents into `~/.uhpm/packages/<name>-<version>`.
/// 5. Create symbolic links if this is a fresh install.
/// 6. Add the package and its files to the database.
/// 7. Mark the installed version as the current version.
///
/// # Errors
/// Returns [`InstallError`] if unpacking, metadata parsing,
/// filesystem operations, or database interactions fail.
pub async fn install(pkg_path: &Path, db: &PackageDB) -> Result<(), InstallError> {
    info!("Starting installation of package: {}", pkg_path.display());

    let unpacked = unpack(pkg_path)?;
    debug!("Archive unpacked into {:?}", unpacked);

    let meta_path = unpacked.join("uhp.ron");
    debug!("Reading metadata from {:?}", meta_path);
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!(
        "Package: {} version {}",
        package_meta.name(),
        package_meta.version()
    );

    let pkg_name = package_meta.name();
    let version = package_meta.version();

    let already_installed = db.is_installed(pkg_name).await.unwrap();
    if let Some(installed_version) = &already_installed {
        info!(
            "Package {} is already installed with version {}",
            pkg_name, installed_version
        );
        if installed_version == version {
            info!("Same version detected — skipping installation");
            return Ok(());
        }
    }

    let package_root = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, version));
    debug!("Package root path: {:?}", package_root);

    if package_root.exists() {
        debug!("Path already exists, removing: {:?}", package_root);
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;
    debug!("Package directory created");

    fs::rename(&unpacked, &package_root)?;
    debug!("Package moved to {:?}", package_root);

    let mut installed_files = Vec::new();
    match already_installed {
        None => {
            info!("Creating symlinks for new package");
            installed_files = create_symlinks(&package_root)?;
        }
        Some(_) => {
            info!("Updating package version — symlinks are not recreated");
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    info!(
        "Adding package {} to database with {} files",
        pkg_name,
        installed_files_str.len()
    );
    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();
    db.set_current_version(&package_meta.name(), &package_meta.version().to_string())
        .await
        .unwrap();

    info!("Package {} installed successfully", pkg_name);
    Ok(())
}

/// Create symbolic links defined in `symlist.ron`.
///
/// Reads a `symlist.ron` file located in the package root and creates
/// links from installed files to their configured destinations.
///
/// # Arguments
/// - `package_root`: Root directory of the unpacked package.
///
/// # Returns
/// A vector of paths to the created symbolic links.
///
/// # Errors
/// - Fails if directories cannot be created.
/// - Fails if existing files cannot be removed.
/// - Fails if symlink creation fails.
pub fn create_symlinks(package_root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut installed_files = Vec::new();

    let symlist_path = package_root.join("symlist.ron");
    debug!("Loading symlist from {:?}", symlist_path);

    match symlist::load_symlist(&symlist_path, &package_root) {
        Ok(symlinks) => {
            for (src_rel, dst_abs) in symlinks {
                let src_abs = package_root.join(&src_rel);
                debug!(
                    "Processing symlink: {} -> {}",
                    src_abs.display(),
                    dst_abs.display()
                );

                if !src_abs.exists() {
                    warn!("Source file not found: {}", src_abs.display());
                    continue;
                }

                if let Some(parent) = dst_abs.parent() {
                    fs::create_dir_all(parent)?;
                    debug!("Created directory for symlink: {:?}", parent);
                }

                if dst_abs.exists() {
                    fs::remove_file(&dst_abs)?;
                    debug!("Removed existing symlink: {:?}", dst_abs);
                }

                std::os::unix::fs::symlink(&src_abs, &dst_abs)?;
                debug!(
                    "Symlink created: {} -> {}",
                    dst_abs.display(),
                    src_abs.display()
                );
                installed_files.push(dst_abs);
            }
        }
        Err(e) => {
            warn!("Failed to load symlist: {:?}", e);
        }
    }

    debug!("Created {} symlinks", installed_files.len());
    Ok(installed_files)
}

/// Unpack a `.uhp` archive into a temporary directory.
///
/// # Arguments
/// - `pkg_path`: Path to the `.uhp` archive.
///
/// # Returns
/// Path to the temporary unpack directory.
///
/// # Errors
/// - Returns an error if the extension is not `.uhp`.
/// - Returns an error if filesystem operations fail.
/// - Returns an error if the archive cannot be decompressed.
///
/// # Notes
/// The unpack directory is always cleared before extraction.
fn unpack(pkg_path: &Path) -> Result<PathBuf, std::io::Error> {
    if pkg_path.extension().and_then(|s| s.to_str()) != Some("uhp") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Package must have .uhp extension",
        ));
    }

    let tmp_dir = dirs::home_dir().unwrap().join(".uhpm/tmp");
    fs::create_dir_all(&tmp_dir)?;

    let package_name = pkg_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown_package");
    let unpack_dir = tmp_dir.join(package_name);

    if unpack_dir.exists() {
        fs::remove_dir_all(&unpack_dir)?;
    }
    fs::create_dir_all(&unpack_dir)?;

    debug!("Unpacking {} into {:?}", pkg_path.display(), unpack_dir);

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    debug!("Unpacked into {:?}", unpack_dir);
    Ok(unpack_dir)
}
