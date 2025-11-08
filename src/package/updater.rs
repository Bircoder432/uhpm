//! Package update functionality

use crate::db::PackageDB;
use crate::error::UpdaterError;
use crate::fetcher;
use crate::repo::{RepoDB, parse_repos};
use crate::{info, warn};
use semver::Version;
use std::path::Path;

/// Checks for package updates and returns download URL if newer version exists
pub async fn check_for_update(
    pkg_name: &str,
    package_db: &PackageDB,
) -> Result<String, UpdaterError> {
    let installed_version = package_db
        .get_package_version(pkg_name)
        .await?
        .ok_or_else(|| UpdaterError::NotFound(pkg_name.to_string()))?;

    info!(
        "package.updater.installed_version",
        pkg_name, &installed_version
    );

    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path).unwrap();

    let mut latest_url = None;
    let mut latest_version: Option<Version> = None;

    for (repo_name, repo_url) in repos {
        info!("package.updater.checking_repo", &repo_name, &repo_url);

        let repo_path = if repo_url.starts_with("file://") {
            Path::new(repo_url.strip_prefix("file://").unwrap()).to_path_buf()
        } else {
            continue; // Skip non-file repos
        };

        let repo_db = match RepoDB::from_repo_path(&repo_path).await {
            Ok(db) => db,
            Err(e) => {
                warn!("package.updater.repo_load_failed", &repo_name, e);
                continue;
            }
        };

        let pkg_list = match repo_db.list_packages().await {
            Ok(list) => list,
            Err(e) => {
                warn!("package.updater.repo_list_failed", &repo_name, e);
                continue;
            }
        };

        for (name, ver_str, url) in pkg_list {
            if name == pkg_name {
                if let Ok(ver) = Version::parse(&ver_str) {
                    let inst_ver =
                        Version::parse(&installed_version).unwrap_or(Version::new(0, 0, 0));

                    if latest_version.as_ref().map_or(true, |v| &ver > v) {
                        latest_version = Some(ver);
                        latest_url = Some(url);
                        info!(
                            "package.updater.newer_version_found",
                            pkg_name, &ver_str, &repo_name
                        );
                    }
                }
            }
        }
    }

    latest_url.ok_or_else(|| UpdaterError::NoNewVersion(pkg_name.to_string()))
}

/// Checks for updates in all installed packages
pub async fn check_all_updates(
    package_db: &PackageDB,
) -> Result<Vec<(String, String, String, String)>, UpdaterError> {
    let installed_packages = package_db.list_packages().await?;
    let mut updates = Vec::new();

    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path).unwrap();

    for (pkg_name, installed_version, _) in installed_packages {
        let mut latest_version: Option<Version> = None;
        let mut latest_repo = String::new();

        for (repo_name, repo_url) in &repos {
            let repo_path = if repo_url.starts_with("file://") {
                Path::new(repo_url.strip_prefix("file://").unwrap()).to_path_buf()
            } else {
                continue;
            };

            let repo_db = match RepoDB::from_repo_path(&repo_path).await {
                Ok(db) => db,
                Err(_) => continue,
            };

            let pkg_list = match repo_db.list_packages().await {
                Ok(list) => list,
                Err(_) => continue,
            };

            for (name, ver_str, _) in pkg_list {
                if name == pkg_name {
                    if let Ok(ver) = Version::parse(&ver_str) {
                        let inst_ver =
                            Version::parse(&installed_version).unwrap_or(Version::new(0, 0, 0));

                        if latest_version.as_ref().map_or(true, |v| &ver > v) {
                            latest_version = Some(ver);
                            latest_repo = repo_name.clone();
                        }
                    }
                }
            }
        }

        if let Some(latest_ver) = latest_version {
            updates.push((
                pkg_name,
                installed_version,
                latest_ver.to_string(),
                latest_repo,
            ));
        }
    }

    Ok(updates)
}

/// Updates package from local file
pub async fn update_from_file(
    pkg_path: &Path,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), UpdaterError> {
    info!("package.updater.updating_from_file", pkg_path.display());

    let url = format!("file://{}", pkg_path.display());
    fetcher::fetch_and_install_parallel(&[url], package_db, direct).await?;

    info!(
        "package.updater.update_from_file_success",
        pkg_path.display()
    );
    Ok(())
}

/// Updates package to latest version available in repositories
pub async fn update_package(
    pkg_name: &str,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), UpdaterError> {
    info!("package.updater.starting_update", pkg_name);

    let download_url = check_for_update(pkg_name, package_db).await?;
    info!(
        "package.updater.downloading_update",
        pkg_name, &download_url
    );

    fetcher::fetch_and_install_parallel(&[download_url], package_db, direct).await?;
    info!("package.updater.update_success", pkg_name);

    Ok(())
}

/// Updates all packages that have newer versions available
pub async fn update_all_packages(package_db: &PackageDB, direct: bool) -> Result<(), UpdaterError> {
    let updates = check_all_updates(package_db).await?;

    if updates.is_empty() {
        info!("package.updater.no_updates_available");
        return Ok(());
    }

    info!("package.updater.updates_found", updates.len());

    for (pkg_name, current_version, new_version, repo_name) in updates {
        info!(
            "package.updater.updating_package",
            &pkg_name, &current_version, &new_version, &repo_name
        );

        if let Err(e) = update_package(&pkg_name, package_db, direct).await {
            warn!("package.updater.update_failed", &pkg_name, e);
        }
    }

    info!("package.updater.all_updates_completed");
    Ok(())
}
