use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;
use tokio::task;
use tracing::{error, info};

use crate::db::PackageDB;
use crate::package::installer;

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Installer error: {0}")]
    Installer(String),
}

/// Скачивание одного пакета (локального или удалённого)
async fn download_package(url: &str) -> Result<PathBuf, FetchError> {
    if let Some(stripped) = url.strip_prefix("file://") {
        Ok(PathBuf::from(stripped))
    } else {
        let resp = reqwest::get(url).await?.bytes().await?;
        let tmp_dir = std::env::temp_dir();
        let filename = Path::new(url)
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| {
                FetchError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Невозможно определить имя файла из URL",
                ))
            })?;
        let tmp_path = tmp_dir.join(filename);
        fs::write(&tmp_path, &resp).await?;
        Ok(tmp_path)
    }
}

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
                bar.set_message(format!("Скачано: {}", url));
            }
            Err(e) => {
                error!("Ошибка при скачивании {}: {}", url, e);
                bar.inc(1);
            }
        }
    }
    bar.finish_with_message("Скачивание завершено");
    results
}

/// Последовательная установка скачанных пакетов
pub async fn install_fetched_packages(
    packages: &HashMap<String, PathBuf>,
    package_db: &PackageDB,
) -> Result<(), FetchError> {
    for (url, path) in packages {
        info!("Установка пакета с {}...", url);
        installer::install(path, package_db).await.map_err(|e| {
            FetchError::Installer(format!("Ошибка установки пакета {}: {:?}", url, e))
        })?;
    }
    Ok(())
}


pub async fn fetch_and_install_parallel(
    urls: &[String],
    package_db: &PackageDB,
) -> Result<(), FetchError> {
    let downloaded = fetch_packages(urls).await;
    install_fetched_packages(&downloaded, package_db).await?;
    Ok(())
}
