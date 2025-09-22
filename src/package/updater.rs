//! # Package Updater
//!
//! This module provides functionality to check for and install newer versions
//! of installed packages from configured repositories.

use crate::db::PackageDB;
use crate::error::UpdaterError;
use crate::fetcher;
use crate::repo::{RepoDB, parse_repos};
use crate::{error, info, warn};
use semver::Version;
use std::path::Path;

/// Errors that may occur during package update.

/// Check for updates and return download URL if newer version exists
pub async fn check_for_update(
    pkg_name: &str,
    package_db: &PackageDB,
) -> Result<String, UpdaterError> {
    // Step 1: check installed version
    let installed_version = package_db.get_package_version(pkg_name).await?;
    if installed_version.is_none() {
        warn!("package.updater.package_not_installed", pkg_name);
        return Err(UpdaterError::NotFound(pkg_name.to_string()));
    }

    let installed_version = installed_version.unwrap();
    info!(
        "package.updater.installed_version",
        pkg_name, &installed_version
    );

    // Step 2: parse repository configuration
    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path).unwrap();

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
            warn!("package.updater.repo_db_not_found", repo_name);
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
                    latest_url = Some(repo_db.get_package(&name, &ver_str).await.unwrap());
                }
            }
        }
    }

    // Return URL if newer version found
    latest_url.ok_or_else(|| UpdaterError::NoNewVersion(pkg_name.to_string()))
}

/// Update package from local file
pub async fn update_from_file(pkg_path: &Path, package_db: &PackageDB) -> Result<(), UpdaterError> {
    info!("package.updater.updating_from_file", pkg_path.display());

    // Convert Path to string URL for fetcher
    let url = format!("file://{}", pkg_path.display());

    // Фетчер сам должен уметь извлекать имя пакета из метаданных
    fetcher::fetch_and_install_parallel(&[url], package_db).await?;

    info!(
        "package.updater.update_from_file_success",
        pkg_path.display()
    );
    Ok(())
}

/// Update a package to the latest version available in repositories.
pub async fn update_package(pkg_name: &str, package_db: &PackageDB) -> Result<(), UpdaterError> {
    // Check for updates
    let download_url = check_for_update(pkg_name, package_db).await?;

    // Download and install
    fetcher::fetch_and_install_parallel(&[download_url], package_db).await?;
    info!("package.updater.update_success", pkg_name);

    Ok(())
}
