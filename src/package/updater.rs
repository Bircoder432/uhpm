//! # Package Updater
//!
//! This module provides functionality to check for and install newer versions
//! of installed packages from configured repositories.

use crate::db::PackageDB;
use crate::error::UpdaterError;
use crate::fetcher;
use crate::repo::{RepoDB, parse_repos};
use crate::{info, warn};
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
    for (repo_name, repo_url) in repos {
        info!("package.updater.checking_repo", &repo_name, &repo_url);

        // Определяем путь к репозиторию
        let repo_path = if repo_url.starts_with("file://") {
            Path::new(repo_url.strip_prefix("file://").unwrap()).to_path_buf()
        } else if repo_url.starts_with("http://") || repo_url.starts_with("https://") {
            // Для HTTP репозиториев используем базовый URL
            // База данных должна быть доступна по {repo_url}/repository.db
            continue; // Пропускаем HTTP пока что, нужна дополнительная логика
        } else {
            // Прямой путь
            Path::new(&repo_url).to_path_buf()
        };

        let repo_db_path = repo_path.join("repository.db");

        // if !repo_db_path.exists() {
        //     warn!("package.updater.repo_db_not_found", &repo_name);
        //     continue;
        // }

        // Используем наш новый метод для загрузки репозитория
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

        // Ищем пакеты в репозитории
        for (name, ver_str, url) in pkg_list {
            if name == pkg_name {
                match Version::parse(&ver_str) {
                    Ok(ver) => {
                        let inst_ver =
                            Version::parse(&installed_version).unwrap_or(Version::new(0, 0, 0));

                        // Используем clone для сравнения без перемещения
                        let current_latest = latest_version.as_ref();
                        if current_latest.is_none() || &ver > current_latest.unwrap() {
                            latest_version = Some(ver);
                            latest_url = Some(url);
                            info!(
                                "package.updater.newer_version_found",
                                pkg_name, &ver_str, &repo_name
                            );
                        }
                    }
                    Err(e) => {
                        warn!("package.updater.version_parse_failed", &ver_str, e);
                        continue;
                    }
                }
            }
        }
    }

    // Return URL if newer version found
    latest_url.ok_or_else(|| UpdaterError::NoNewVersion(pkg_name.to_string()))
}

/// Check for updates in all installed packages
pub async fn check_all_updates(
    package_db: &PackageDB,
) -> Result<Vec<(String, String, String, String)>, UpdaterError> {
    // Получаем список всех установленных пакетов
    let installed_packages = package_db.list_packages().await?;
    let mut updates = Vec::new();

    // Парсим конфигурацию репозиториев
    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path).unwrap();

    for (pkg_name, installed_version, _) in installed_packages {
        let mut latest_version: Option<Version> = None;
        let mut latest_repo = String::new();

        for (repo_name, repo_url) in &repos {
            let repo_path = if repo_url.starts_with("file://") {
                Path::new(repo_url.strip_prefix("file://").unwrap()).to_path_buf()
            } else {
                continue; // Пропускаем не-file репозитории для упрощения
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

                        // Используем as_ref для сравнения без перемещения
                        let current_latest = latest_version.as_ref();
                        if current_latest.is_none() || &ver > current_latest.unwrap() {
                            latest_version = Some(ver);
                            latest_repo = repo_name.clone();
                        }
                    }
                }
            }
        }

        if let Some(latest_ver) = latest_version {
            updates.push((
                pkg_name.clone(),
                installed_version,
                latest_ver.to_string(),
                latest_repo.clone(),
            ));
        }
    }

    Ok(updates)
}

/// Update package from local file
pub async fn update_from_file(
    pkg_path: &Path,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), UpdaterError> {
    info!("package.updater.updating_from_file", pkg_path.display());

    // Convert Path to string URL for fetcher
    let url = format!("file://{}", pkg_path.display());

    // Фетчер сам должен уметь извлекать имя пакета из метаданных
    fetcher::fetch_and_install_parallel(&[url], package_db, direct).await?;

    info!(
        "package.updater.update_from_file_success",
        pkg_path.display()
    );
    Ok(())
}

/// Update a package to the latest version available in repositories.
pub async fn update_package(
    pkg_name: &str,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), UpdaterError> {
    info!("package.updater.starting_update", pkg_name);

    // Check for updates
    let download_url = check_for_update(pkg_name, package_db).await?;

    info!(
        "package.updater.downloading_update",
        pkg_name, &download_url
    );

    // Download and install
    fetcher::fetch_and_install_parallel(&[download_url], package_db, direct).await?;
    info!("package.updater.update_success", pkg_name);

    Ok(())
}

/// Update all packages that have newer versions available
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
