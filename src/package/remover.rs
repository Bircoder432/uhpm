//! # Package Remover Module
//!
//! This module provides functionality for removing installed UHPM packages.
//! It handles package file cleanup, symlink removal, and database record deletion.
//!
//! ## Main Components
//!
//! - [`DeleteError`]: Enumeration of possible removal errors
//! - [`remove()`]: Main function for package removal
//!
//! ## Removal Process
//!
//! 1. **Database Check**: Verifies if package exists in database
//! 2. **Directory Removal**: Deletes package installation directory
//! 3. **File Cleanup**: Removes all installed files and symlinks
//! 4. **Database Update**: Removes package record from database
//!
//! ## Error Handling
//!
//! Errors are categorized into I/O errors and database errors,
//! both wrapped in the [`DeleteError`] enumeration.

use crate::db::PackageDB;
use crate::{info, warn};

/// Errors that can occur during package removal
#[derive(Debug)]
pub enum DeleteError {
    /// I/O error during file operations
    Io(std::io::Error),
    /// Database error during record deletion
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

/// Removes an installed package and all its associated files
///
/// # Arguments
/// * `pkg_name` - Name of the package to remove
/// * `db` - Reference to the package database
///
/// # Returns
/// `Result<(), DeleteError>` - Success or error result
///
/// # Process
/// 1. Checks if package exists in database
/// 2. Removes package installation directory
/// 3. Removes all installed files and symlinks
/// 4. Deletes package record from database
///
/// # Notes
/// - If package directory doesn't exist, removal continues with file cleanup
/// - Non-existent files are skipped during cleanup
/// - Database record is always removed if package exists in database
pub async fn remove(pkg_name: &str, db: &PackageDB) -> Result<(), DeleteError> {
    let version = db.get_package_version(pkg_name).await?;
    if version.is_none() {
        warn!("uhpm.remove.pkg_not_found_db", pkg_name);
        return Ok(());
    }
    let version = version.unwrap();

    remove_by_version(pkg_name, &version, db).await?;
    Ok(())
}

pub async fn remove_by_version(
    pkg_name: &str,
    version: &str,
    db: &PackageDB,
) -> Result<(), DeleteError> {
    info!("uhpm.remove.attempting_remove", pkg_name, &version);

    let mut pkg_dir = dirs::home_dir().unwrap();
    pkg_dir.push(".uhpm/packages");
    pkg_dir.push(format!("{}-{}", pkg_name, version));

    if pkg_dir.exists() {
        std::fs::remove_dir_all(&pkg_dir)?;
        info!("uhpm.remove.pkg_dir_removed", pkg_dir.display());
    } else {
        warn!("uhpm.remove.pkg_dir_not_found", pkg_name, pkg_dir.display());
    }

    let files: Vec<String> = db.get_installed_files(pkg_name).await?;
    for f in files {
        let path = std::path::PathBuf::from(f);
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            info!("uhpm.remove.file_removed", path.display());
        }
    }

    db.remove_package(pkg_name).await?;
    info!("uhpm.remove.pkg_entry_removed", pkg_name);

    Ok(())
}
