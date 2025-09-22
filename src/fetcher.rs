//! # Package Fetcher
//!
//! This module handles downloading packages from URLs (HTTP or local `file://` paths)
//! and installing them into UHPM.
//!
//! ## Responsibilities
//! - Download `.uhp` package archives from repositories or local paths.
//! - Provide progress bars for concurrent downloads.
//! - Integrate with the [`installer`](crate::package::installer) to complete installation.
//!
//! ## Features
//! - Supports both **HTTP(S)** and **file://** sources.
//! - Parallel downloading using [`FuturesUnordered`].
//! - Progress display via [`indicatif`].
//! - Error handling through [`FetchError`].
//!
//! ## Example
//! ```rust,no_run
//! use uhpm::db::PackageDB;
//! use uhpm::fetcher::fetch_and_install_parallel;
//! # use std::path::Path;
//!
//! # tokio_test::block_on(async {
//! let db = PackageDB::new(Path::new("/tmp/uhpm.db"))
//!     .unwrap()
//!     .init()
//!     .await
//!     .unwrap();
//!
//! let urls = vec![
//!     "https://example.com/package.uhp".to_string(),
//!     "file:///home/user/package.uhp".to_string()
//! ];
//!
//! fetch_and_install_parallel(&urls, &db).await.unwrap();
//! # });
//! ```

use crate::db::PackageDB;
use crate::error::FetchError;
use crate::package::installer;
use crate::{error, info};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Errors that can occur during package fetching and installation.

/// Downloads a single package from the given URL.
///
/// - Supports both `http(s)://` and `file://` schemes.
/// - For local `file://` paths, simply converts to [`PathBuf`].
/// - For remote URLs, saves the response to the temporary directory.
///
/// # Errors
/// Returns a [`FetchError`] if the request or file writing fails.
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
                    "Unable to determine filename from URL",
                ))
            })?;
        let tmp_path = tmp_dir.join(filename);
        fs::write(&tmp_path, &resp).await?;
        Ok(tmp_path)
    }
}

/// Downloads multiple packages concurrently.
///
/// Shows a progress bar using [`indicatif`] while downloading.
/// Returns a map of successfully downloaded URLs to local file paths.
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

/// Installs already downloaded packages into the database.
///
/// # Errors
/// Returns [`FetchError::Installer] if the installation fails.
pub async fn install_fetched_packages(
    packages: &HashMap<String, PathBuf>,
    package_db: &PackageDB,
) -> Result<(), FetchError> {
    for (url, path) in packages {
        info!("fetcher.install.from_url", url);
        installer::install(path, package_db).await.map_err(|e| {
            FetchError::Installer(format!("Installation failed for {}: {:?}", url, e))
        })?;
    }
    Ok(())
}

/// Downloads and installs packages in parallel.
///
/// - Downloads all URLs concurrently.
/// - Installs them sequentially after downloading.
///
/// # Errors
/// Returns [`FetchError`] if downloading or installation fails.
pub async fn fetch_and_install_parallel(
    urls: &[String],
    package_db: &PackageDB,
) -> Result<(), FetchError> {
    let downloaded = fetch_packages(urls).await;
    install_fetched_packages(&downloaded, package_db).await?;
    Ok(())
}
