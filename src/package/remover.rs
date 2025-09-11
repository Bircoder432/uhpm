//! # Package Remover
//!
//! This module provides functionality to uninstall packages that were
//! previously installed by UHPM. It ensures both the package files and
//! the corresponding database records are removed cleanly.
//!
//! ## Responsibilities
//! - Look up installed package version in the database.
//! - Remove the package directory from `~/.uhpm/packages`.
//! - Remove all installed files recorded in the database.
//! - Remove the package entry from the database.
//!
//! Errors are unified under [`DeleteError`] for consistency.

use crate::db::PackageDB;

/// Errors that may occur when removing a package.
#[derive(Debug)]
pub enum DeleteError {
    /// Filesystem or I/O error.
    Io(std::io::Error),
    /// Database error from `sqlx`.
    Db(sqlx::Error),
}

impl From<std::io::Error> for DeleteError {
    fn from(e: std::io::Error) -> Self {
        DeleteError::Io(e)
    }
}

impl From<sqlx::Error> for DeleteError {
    fn from(e: sqlx::Error) -> Self {
        DeleteError::Db(e)
    }
}

/// Remove a package from the system and the database.
///
/// # Arguments
/// - `pkg_name`: The name of the package to remove.
/// - `db`: Reference to the [`PackageDB`] instance.
///
/// # Workflow
/// 1. Look up the installed version of the package in the database.
///    - If the package is not installed, logs a warning and returns `Ok(())`.
/// 2. Remove the package directory from `~/.uhpm/packages/<name>-<version>`.
/// 3. Remove all files listed in the database under `installed_files`.
/// 4. Delete the package entry and dependencies from the database.
///
/// # Errors
/// Returns [`DeleteError`] if:
/// - Filesystem operations (removing directories or files) fail.
/// - Database queries fail.
///
/// # Logging
/// - Logs warnings if the package or its directory is not found.
/// - Logs each successfully removed file or directory.
pub async fn remove(pkg_name: &str, db: &PackageDB) -> Result<(), DeleteError> {
    // Retrieve installed version from the database
    let version = db.get_package_version(pkg_name).await?;
    if version.is_none() {
        tracing::warn!("Package '{}' not found in the database", pkg_name);
        return Ok(());
    }
    let version = version.unwrap();

    tracing::info!("Attempting to remove package: {}-{}", pkg_name, version);

    // Construct path to package directory
    let mut pkg_dir = dirs::home_dir().unwrap();
    pkg_dir.push(".uhpm/packages");
    pkg_dir.push(format!("{}-{}", pkg_name, version));

    // Remove package directory if it exists
    if pkg_dir.exists() {
        std::fs::remove_dir_all(&pkg_dir)?;
        tracing::info!("Removed package directory: {}", pkg_dir.display());
    } else {
        tracing::warn!(
            "Package directory '{}' not found: {}",
            pkg_name,
            pkg_dir.display()
        );
    }

    // Remove all installed files recorded in the database
    let files: Vec<String> = db.get_installed_files(pkg_name).await?;
    for f in files {
        let path = std::path::PathBuf::from(f);
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            tracing::info!("Removed: {}", path.display());
        }
    }

    // Remove package entry from database
    db.remove_package(pkg_name).await?;
    tracing::info!("Package '{}' entry removed from database", pkg_name);

    Ok(())
}
