use crate::db::PackageDB;
use crate::error::FetchError;
use crate::package::installer;
use crate::{error, info};
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Validates and sanitizes file paths to prevent directory traversal attacks
fn sanitize_file_path(path: &str) -> Result<PathBuf, FetchError> {
    let path_buf = PathBuf::from(path);

    if path_buf
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(FetchError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Path contains parent directory references",
        )));
    }

    Ok(path_buf)
}

/// Downloads package from URL with security validation
async fn download_package(url: &str) -> Result<PathBuf, FetchError> {
    if let Some(stripped) = url.strip_prefix("file://") {
        let sanitized_path = sanitize_file_path(stripped)?;
        if !sanitized_path.exists() {
            return Err(FetchError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not found",
            )));
        }
        Ok(sanitized_path)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| FetchError::Http(e.into()))?;

        let resp = client.get(url).send().await?.bytes().await?;
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

        let safe_filename = sanitize_file_path(filename)?;
        let tmp_path = tmp_dir.join(safe_filename);
        fs::write(&tmp_path, &resp).await?;
        Ok(tmp_path)
    } else {
        let safe_path = sanitize_file_path(url)?;
        Ok(safe_path)
    }
}

/// Downloads build scripts for source packages with security checks
pub async fn download_source_build_script(url: &str) -> Result<PathBuf, FetchError> {
    if let Some(stripped) = url.strip_prefix("file://") {
        let sanitized_path = sanitize_file_path(stripped)?;
        Ok(sanitized_path)
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| FetchError::Http(e.into()))?;

        let resp = client.get(url).send().await?.bytes().await?;
        let tmp_dir = std::env::temp_dir();
        let filename = "uhpbuild.sh";
        let tmp_path = tmp_dir.join(filename);
        fs::write(&tmp_path, &resp).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&tmp_path).await?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp_path, perms).await?;
        }

        Ok(tmp_path)
    } else {
        let safe_path = sanitize_file_path(url)?;
        Ok(safe_path)
    }
}

/// Downloads multiple packages in parallel with progress tracking
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

/// Installs downloaded packages with transaction safety
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

/// Downloads and installs packages in parallel with error handling
pub async fn fetch_and_install_parallel(
    urls: &[String],
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), FetchError> {
    let downloaded = fetch_packages(urls).await;
    install_fetched_packages(&downloaded, package_db, direct).await?;
    Ok(())
}

/// Fetches package from repository by name and version with validation
pub async fn fetch_package_from_repo(
    repo_db: &crate::repo::RepoDB,
    package_name: &str,
    package_version: &str,
    package_db: &PackageDB,
    direct: bool,
) -> Result<(), FetchError> {
    let package_url = repo_db
        .get_package_url(package_name, package_version)
        .await
        .map_err(|_| FetchError::Installer("Package not found in repository".to_string()))?;

    info!(
        "fetcher.found_package",
        package_name, package_version, &package_url
    );

    let urls = vec![package_url];
    fetch_and_install_parallel(&urls, package_db, direct).await?;

    Ok(())
}

/// Fetches sources for package build with security checks
pub async fn fetch_sources_for_build(
    repo_db: &crate::repo::RepoDB,
    package_name: &str,
    package_version: &str,
) -> Result<PathBuf, FetchError> {
    let source_url = repo_db
        .get_source_url(package_name, package_version)
        .await
        .map_err(|_| FetchError::Installer("Source not found in repository".to_string()))?;

    info!(
        "fetcher.found_sources",
        package_name, package_version, &source_url
    );

    download_source_build_script(&source_url).await
}

/// Downloads file to specific path with directory creation and validation
pub async fn download_file_to_path(url: &str, destination: &Path) -> Result<(), FetchError> {
    info!("fetcher.download_to_path", url, destination.display());

    if let Some(stripped) = url.strip_prefix("file://") {
        let source_path = sanitize_file_path(stripped)?;
        if source_path != destination {
            fs::copy(&source_path, destination).await?;
        }
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(false)
            .build()
            .map_err(|e| FetchError::Http(e.into()))?;

        let resp = client.get(url).send().await?.bytes().await?;
        fs::write(destination, &resp).await?;
    } else {
        let source_path = sanitize_file_path(url)?;
        if source_path != destination {
            fs::copy(&source_path, destination).await?;
        }
    }

    info!("fetcher.download_complete", destination.display());
    Ok(())
}

/// Downloads file creating parent directories with path validation
pub async fn download_file_to_path_with_dirs(
    url: &str,
    destination: &Path,
) -> Result<(), FetchError> {
    let safe_destination = sanitize_file_path(destination.to_str().unwrap_or(""))?;

    if let Some(parent) = safe_destination.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }

    download_file_to_path(url, &safe_destination).await
}
