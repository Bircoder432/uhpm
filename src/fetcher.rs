//! # Package Fetcher
//!
//! This module handles downloading packages from our UHP repositories.

use crate::db::PackageDB;
use crate::error::FetchError;
use crate::package::installer;
use crate::{error, info};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Скачивает пакет из нашего репозитория
async fn download_package(url: &str) -> Result<PathBuf, FetchError> {
    if let Some(stripped) = url.strip_prefix("file://") {
        // Локальный файл
        Ok(PathBuf::from(stripped))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        // HTTP скачивание
        let resp = reqwest::get(url).await?.bytes().await?;
        let tmp_dir = std::env::temp_dir();
        let filename = Path::new(url)
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| {
                FetchError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Unable to determine filename from URL",
                ))
            })?;
        let tmp_path = tmp_dir.join(filename);
        fs::write(&tmp_path, &resp).await?;
        Ok(tmp_path)
    } else {
        // Прямой путь к файлу
        Ok(PathBuf::from(url))
    }
}

/// Скачивает uhpbuild скрипты для сборки из исходников
pub async fn download_source_build_script(url: &str) -> Result<PathBuf, FetchError> {
    if let Some(stripped) = url.strip_prefix("file://") {
        Ok(PathBuf::from(stripped))
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let resp = reqwest::get(url).await?.bytes().await?;
        let tmp_dir = std::env::temp_dir();
        let filename = "uhpbuild.sh"; // Стандартное имя для скрипта сборки
        let tmp_path = tmp_dir.join(filename);
        fs::write(&tmp_path, &resp).await?;

        // Делаем скрипт исполняемым
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&tmp_path).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp_path, perms).await?;
        }

        Ok(tmp_path)
    } else {
        Ok(PathBuf::from(url))
    }
}

/// Скачивает несколько пакетов параллельно
pub async fn fetch_packages(urls: &[String]) -> HashMap<String, PathBuf> {
    let bar = ProgressBar::new(urls.len() as u64);
    bar.set_style(
        ProgressStyle::default_bar()
            .template("[{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("##-"),
    );

    let mut futures = FuturesUnordered::new();
    for url in urls {
        let url_clone = url.clone();
        futures.push(async move {
            let path = download_package(&url_clone).await;
            (url_clone, path)
        });
    }

    let mut results = HashMap::new();
    while let Some((url, res)) = futures.next().await {
        match res {
            Ok(path) => {
                results.insert(url.clone(), path);
                bar.inc(1);
                bar.set_message(format!("Downloaded: {}", url));
            }
            Err(e) => {
                error!("fetcher.download.failed", url, e);
                bar.inc(1);
            }
        }
    }
    bar.finish_with_message("Download complete");
    results
}

/// Устанавливает скачанные пакеты
pub async fn install_fetched_packages(
    packages: &HashMap<String, PathBuf>,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), FetchError> {
    for (url, path) in packages {
        info!("fetcher.install.from_url", url);
        installer::install(path, package_db, direct)
            .await
            .map_err(|e| {
                FetchError::Installer(format!("Installation failed for {}: {:?}", url, e))
            })?;
    }
    Ok(())
}

/// Скачивает и устанавливает пакеты параллельно
pub async fn fetch_and_install_parallel(
    urls: &[String],
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), FetchError> {
    let downloaded = fetch_packages(urls).await;
    install_fetched_packages(&downloaded, package_db, direct).await?;
    Ok(())
}

/// Скачивает пакеты из репозитория по имени и версии
pub async fn fetch_package_from_repo(
    repo_db: &crate::repo::RepoDB,
    package_name: &str,
    package_version: &str,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), FetchError> {
    // Получаем URL пакета из репозитория
    let package_url = repo_db
        .get_package_url(package_name, package_version)
        .await
        .unwrap();

    info!(
        "fetcher.found_package",
        package_name, package_version, &package_url
    );

    // Скачиваем и устанавливаем
    let urls = vec![package_url];
    fetch_and_install_parallel(&urls, package_db, direct).await?;

    Ok(())
}

/// Скачивает исходники для сборки пакета
pub async fn fetch_sources_for_build(
    repo_db: &crate::repo::RepoDB,
    package_name: &str,
    package_version: &str,
) -> Result<PathBuf, FetchError> {
    // Получаем URL исходников из репозитория
    let source_url = repo_db
        .get_source_url(package_name, package_version)
        .await
        .unwrap();

    info!(
        "fetcher.found_sources",
        package_name, package_version, &source_url
    );

    // Скачиваем скрипт сборки
    download_source_build_script(&source_url).await
}

pub async fn download_file_to_path(url: &str, destination: &Path) -> Result<(), FetchError> {
    info!("fetcher.download_to_path", url, destination.display());

    if let Some(stripped) = url.strip_prefix("file://") {
        // Локальный файл - копируем в указанное место
        let source_path = PathBuf::from(stripped);
        if source_path != destination {
            fs::copy(&source_path, destination).await?;
        }
    } else if url.starts_with("http://") || url.starts_with("https://") {
        // HTTP скачивание напрямую в указанный путь
        let resp = reqwest::get(url).await?.bytes().await?;
        fs::write(destination, &resp).await?;
    } else {
        // Прямой путь к файлу - копируем если пути разные
        let source_path = PathBuf::from(url);
        if source_path != destination {
            fs::copy(&source_path, destination).await?;
        }
    }

    info!("fetcher.download_complete", destination.display());
    Ok(())
}

/// Скачивает файл по ссылке в указанный путь с созданием родительских директорий
pub async fn download_file_to_path_with_dirs(
    url: &str,
    destination: &Path,
) -> Result<(), FetchError> {
    // Создаем родительские директории если нужно
    if let Some(parent) = destination.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }

    download_file_to_path(url, destination).await
}
