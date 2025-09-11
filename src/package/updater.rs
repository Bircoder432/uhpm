//! # Package Updater
//!
//! This module provides functionality to check for and install newer versions
//! of installed packages from configured repositories.
//!
//! ## Responsibilities
//! - Determine the currently installed version of a package.
//! - Parse the repository configuration (`repos.ron`).
//! - Query repositories to find the latest available version.
//! - Download and install the newer version if available.
//!
//! Errors are unified under [`UpdaterError`] for consistency.

use crate::db::PackageDB;
use crate::fetcher;
use crate::repo::{RepoDB, RepoError, parse_repos};
use semver::Version;
use tracing::{error, info, warn};

/// Errors that may occur during package update.
#[derive(thiserror::Error, Debug)]
pub enum UpdaterError {
    /// The package is not installed and therefore cannot be updated.
    #[error("Package not found: {0}")]
    NotFound(String),

    /// Filesystem or I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error while working with repository database or configuration.
    #[error("Repo error: {0}")]
    Repo(#[from] RepoError),

    /// Database error from `sqlx`.
    #[error("DB error: {0}")]
    Db(#[from] sqlx::Error),

    /// Error during fetch or installation of the new package.
    #[error("Fetch error: {0}")]
    Fetch(#[from] crate::fetcher::FetchError),
}

/// Update a package to the latest version available in repositories.
///
/// # Arguments
/// - `pkg_name`: Name of the package to update.
/// - `package_db`: Reference to the [`PackageDB`] instance.
///
/// # Workflow
/// 1. Look up the currently installed version in the database.
///    - If the package is not installed, return [`UpdaterError::NotFound`].
/// 2. Parse the repository configuration (`~/.uhpm/repos.ron`).
/// 3. For each repository:
///    - Open its `packages.db`.
///    - Check if a newer version of the package exists.
///    - Track the highest available version and its download URL.
/// 4. If a newer version is found:
///    - Download and install it via [`fetcher::fetch_and_install_parallel`].
///    - Log the update success.
/// 5. If no newer version is found:
///    - Log that the package is up to date.
///
/// # Errors
/// Returns [`UpdaterError`] if:
/// - The package is not installed.
/// - Repository parsing or database queries fail.
/// - Filesystem or network operations fail.
/// - Installation of the new version fails.
///
/// # Logging
/// - Logs the installed version and the found latest version.
/// - Logs update progress and success/failure.
pub async fn update_package(pkg_name: &str, package_db: &PackageDB) -> Result<(), UpdaterError> {
    // Step 1: check installed version
    let installed_version = package_db.get_package_version(pkg_name).await?;
    if installed_version.is_none() {
        warn!("Package {} is not installed", pkg_name);
        return Err(UpdaterError::NotFound(pkg_name.to_string()));
    }

    let installed_version = installed_version.unwrap();
    info!("Installed version of {}: {}", pkg_name, installed_version);

    // Step 2: parse repository configuration
    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path)?;

    let mut latest_url = None;
    let mut latest_version: Option<Version> = None;

    // Step 3: iterate through repositories
    for (repo_name, repo_path) in repos {
        let repo_path = repo_path
            .strip_prefix("file://")
            .unwrap_or(&repo_path)
            .to_string();
        let repo_db_path = std::path::Path::new(&repo_path).join("packages.db");

        if !repo_db_path.exists() {
            warn!("Repository database {} not found, skipping", repo_name);
            continue;
        }

        let repo_db = RepoDB::new(&repo_db_path).await?;
        let pkg_list = repo_db.list_packages().await?;

        for (name, ver_str) in pkg_list {
            if name == pkg_name {
                let ver = Version::parse(&ver_str).unwrap_or(Version::new(0, 0, 0));
                let inst_ver = Version::parse(&installed_version).unwrap_or(Version::new(0, 0, 0));
                if ver > inst_ver {
                    latest_version = Some(ver);
                    latest_url = Some(repo_db.get_package(&name, &ver_str).await?);
                }
            }
        }
    }

    // Step 4: install update if available
    if let Some(url) = latest_url {
        info!(
            "New version of {} found: {}",
            pkg_name,
            latest_version.unwrap()
        );
        fetcher::fetch_and_install_parallel(&[url], package_db).await?;
        info!("Package {} updated successfully", pkg_name);
    } else {
        info!("Package {} is already up to date", pkg_name);
    }

    Ok(())
}
