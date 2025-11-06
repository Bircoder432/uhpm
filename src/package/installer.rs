//! # Package Installer Module
//!
//! This module provides functionality for installing UHPM packages from `.uhp` archive files.
//! It handles package extraction, metadata parsing, symlink creation, and database registration.
//!
//! ## Main Components
//!
//! - [`InstallError`]: Enumeration of possible installation errors
//! - [`install()`]: Main installation function for package archives
//! - [`create_symlinks()`]: Creates symbolic links for package files
//! - [`unpack()`]: Extracts package archives to temporary directory
//!
//! ## Installation Process
//!
//! 1. **Extraction**: Package archive is extracted to temporary directory
//! 2. **Metadata Parsing**: Package metadata is read from `uhp.toml` file
//! 3. **Version Check**: Verifies if package is already installed
//! 4. **Directory Setup**: Creates package directory in UHPM home
//! 5. **Symlink Creation**: Creates symbolic links based on `symlist`
//! 6. **Database Registration**: Records package info in package database
//!
//! ## Error Handling
//!
//! Errors are categorized into I/O errors and metadata parsing errors,
//! both wrapped in the [`InstallError`] enumeration.

use crate::db::PackageDB;
use crate::error::UhpmError;
use crate::package::Package;
use crate::symlist;
use crate::{debug, info, warn};
use flate2::read::GzDecoder;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;

/// Errors that can occur during package installation
#[derive(Debug)]
pub enum InstallError {
    /// I/O error during file operations
    Io(std::io::Error),
    /// Error parsing package metadata
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

/// Installs a package from a `.uhp` archive file
///
/// # Arguments
/// * `pkg_path` - Path to the package archive file
/// * `db` - Reference to the package database
///
/// # Returns
/// `Result<(), InstallError>` - Success or error result
///
/// # Process
/// 1. Extracts package to temporary directory
/// 2. Parses package metadata from `uhp.toml`
/// 3. Checks if package is already installed
/// 4. Moves package to permanent location
/// 5. Creates symbolic links for package files
/// 6. Updates package database
pub async fn install(pkg_path: &Path, db: &PackageDB, direct: bool) -> Result<(), UhpmError> {
    info!("installer.install.starting", pkg_path.display());

    let unpacked = unpack(pkg_path)?;
    debug!("installer.install.unpacked", unpacked.display());

    let meta_path = unpacked.join("uhp.toml");
    debug!("installer.install.reading_meta", meta_path.display());
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!(
        "installer.install.package_info",
        package_meta.name(),
        package_meta.version()
    );

    let pkg_name = package_meta.name();
    let version = package_meta.version();

    let already_installed = db.is_installed(pkg_name).await.unwrap();
    if let Some(installed_version) = &already_installed {
        info!(
            "installer.install.already_installed",
            pkg_name, installed_version
        );
        if installed_version == version {
            info!("installer.install.same_version_skipped");
            return Ok(());
        }
    }

    let package_root = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, version));
    debug!("installer.install.package_root", package_root.display());

    if package_root.exists() {
        debug!(
            "installer.install.removing_existing",
            package_root.display()
        );
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;
    debug!("installer.install.created_dir", package_root.display());

    fs::rename(&unpacked, &package_root)?;
    debug!("installer.install.moved_package", package_root.display());

    let mut installed_files = Vec::new();
    match already_installed {
        None => {
            info!("installer.install.creating_symlinks");
            installed_files = create_symlinks(&package_root, direct)?;
        }
        Some(_) => {
            info!("installer.install.updating_version");
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    info!(
        "installer.install.adding_to_db",
        pkg_name,
        installed_files_str.len()
    );
    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();
    db.set_current_version(&package_meta.name(), &package_meta.version().to_string())
        .await
        .unwrap();

    info!("installer.install.success", pkg_name);
    Ok(())
}

/// Creates symbolic links for package files based on symlist configuration
///
/// # Arguments
/// * `package_root` - Path to the package directory
///
/// # Returns
/// `Result<Vec<PathBuf>, std::io::Error>` - List of created symlink paths or error
///
/// # Process
/// 1. Loads symlink configuration from `symlist`
/// 2. Creates parent directories for symlink targets
/// 3. Removes existing files at target locations
/// 4. Creates symbolic links from package files to target locations

pub fn create_symlinks(package_root: &Path, direct: bool) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut installed_files = Vec::new();

    let symlist_path = package_root.join("symlist");
    debug!("installer.symlinks.loading", symlist_path.display());

    match symlist::load_symlist(&symlist_path, &package_root) {
        Ok(symlinks) => {
            for (src_rel, dst_abs) in symlinks {
                let src_abs = package_root.join(&src_rel);
                debug!(
                    "installer.symlinks.processing",
                    src_abs.display(),
                    dst_abs.display()
                );

                if !src_abs.exists() {
                    warn!("installer.symlinks.src_not_found", src_abs.display());
                    continue;
                }

                if let Some(parent) = dst_abs.parent() {
                    fs::create_dir_all(parent)?;
                    debug!("installer.symlinks.created_parent", parent.display());
                }

                if dst_abs.exists() {
                    fs::remove_file(&dst_abs)?;
                    debug!("installer.symlinks.removed_existing", dst_abs.display());
                }
                if direct {
                    std::fs::copy(&src_abs, &dst_abs)?;
                } else {
                    std::os::unix::fs::symlink(&src_abs, &dst_abs)?;
                }
                debug!(
                    "installer.symlinks.created_link",
                    dst_abs.display(),
                    src_abs.display()
                );
                installed_files.push(dst_abs);
            }
        }
        Err(e) => {
            warn!("installer.symlinks.load_failed", e);
        }
    }

    debug!("installer.symlinks.total_created", installed_files.len());
    Ok(installed_files)
}

/// Extracts a package archive to a temporary directory
///
/// # Arguments
/// * `pkg_path` - Path to the package archive file
///
/// # Returns
/// `Result<PathBuf, std::io::Error>` - Path to extracted directory or error
///
/// # Process
/// 1. Validates file extension (.uhp)
/// 2. Creates temporary extraction directory
/// 3. Extracts tar.gz archive contents
/// 4. Returns path to extracted directory
pub fn unpack(pkg_path: &Path) -> Result<PathBuf, std::io::Error> {
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

    debug!(
        "installer.unpack.unpacking",
        pkg_path.display(),
        unpack_dir.display()
    );

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    debug!("installer.unpack.done", unpack_dir.display());
    Ok(unpack_dir)
}

pub async fn install_at(
    pkg_path: &Path,
    db: &PackageDB,
    uhpm_root: &Path,
    direct: bool,
) -> Result<(), crate::package::installer::InstallError> {
    info!("installer.install_at.starting", pkg_path.display());

    let unpacked = unpack_at(pkg_path, uhpm_root)?;
    debug!("installer.install_at.unpacked", unpacked.display());

    let meta_path = unpacked.join("uhp.toml"); // Исправлено: uhp.ron -> uhp.toml
    debug!("installer.install_at.reading_meta", meta_path.display());
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!(
        "installer.install_at.package_info",
        package_meta.name(),
        package_meta.version()
    );

    let pkg_name = package_meta.name();
    let version = package_meta.version();

    let already_installed = db.is_installed(pkg_name).await.unwrap();
    if let Some(installed_version) = &already_installed {
        info!(
            "installer.install_at.already_installed",
            pkg_name, installed_version
        );
        if installed_version == version {
            info!("installer.install_at.same_version_skipped");
            return Ok(());
        }
    }

    let package_root = uhpm_root
        .join("packages")
        .join(format!("{}-{}", pkg_name, version));
    debug!("installer.install_at.package_root", package_root.display());

    if package_root.exists() {
        debug!(
            "installer.install_at.removing_existing",
            package_root.display()
        );
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;
    debug!("installer.install_at.created_dir", package_root.display());

    fs::rename(&unpacked, &package_root)?;
    debug!("installer.install_at.moved_package", package_root.display());

    let mut installed_files = Vec::new();
    match already_installed {
        None => {
            info!("installer.install_at.creating_symlinks");
            installed_files = create_symlinks(&package_root, direct)?;
        }
        Some(_) => {
            info!("installer.install_at.updating_version");
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    info!(
        "installer.install_at.adding_to_db",
        pkg_name,
        installed_files_str.len()
    );
    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();
    db.set_current_version(&package_meta.name(), &package_meta.version().to_string())
        .await
        .unwrap();

    info!("installer.install_at.success", pkg_name);
    Ok(())
}

/// Распаковка пакета в указанную директорию UHPM
pub fn unpack_at(pkg_path: &Path, uhpm_root: &Path) -> Result<PathBuf, std::io::Error> {
    if pkg_path.extension().and_then(|s| s.to_str()) != Some("uhp") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Package must have .uhp extension",
        ));
    }

    let tmp_dir = uhpm_root.join("tmp");
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

    debug!(
        "installer.unpack_at.unpacking",
        pkg_path.display(),
        unpack_dir.display()
    );

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    debug!("installer.unpack_at.done", unpack_dir.display());
    Ok(unpack_dir)
}
