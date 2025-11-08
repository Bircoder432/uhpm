use crate::db::PackageDB;
use crate::error::{ConfigError, UhpmError};
use crate::fetcher;
use crate::package::{installer, remover, switcher, updater};
use crate::repo::{RepoDB, cache_repo, parse_repos};
use semver::Version;
use std::path::{Path, PathBuf};

/// Package management service
pub struct PackageService {
    db: PackageDB,
}

impl PackageService {
    pub fn new(db: PackageDB) -> Self {
        Self { db }
    }

    /// Installs package from file
    pub async fn install_from_file(&self, path: &Path, direct: bool) -> Result<(), UhpmError> {
        installer::install(path, &self.db, direct).await?;
        Ok(())
    }

    /// Extracts package without installing
    pub async fn extract_package(&self, path: &Path) -> Result<(), UhpmError> {
        installer::unpack(path)?;
        Ok(())
    }

    /// Installs package from repository
    pub async fn install_from_repo(
        &self,
        package_name: &str,
        version: Option<&str>,
        direct: bool,
    ) -> Result<(), UhpmError> {
        let repos = cache_repo(self.load_repositories().await?).await;
        let mut urls_to_download = Vec::new();
        let mut found = false;

        for repo_path in &repos {
            if !repo_path.exists() {
                tracing::warn!("Repository database not found: {}", repo_path.display());
                continue;
            }

            let repo_db = RepoDB::new(&repo_path).await?;
            let packages = repo_db.list_packages().await?;

            for (name, pkg_version, url) in packages {
                if name == package_name {
                    if version.is_none() || version.unwrap() == pkg_version {
                        urls_to_download.push(url);
                        found = true;

                        if version.is_some() {
                            break;
                        }
                    }
                }
            }

            if found && version.is_some() {
                break;
            }
        }

        if urls_to_download.is_empty() {
            return Err(UhpmError::NotFound(format!(
                "Package {} not found in repositories",
                package_name
            )));
        }

        tracing::info!("Found packages to download: {:?}", urls_to_download);
        fetcher::fetch_and_install_parallel(&urls_to_download, &self.db, direct).await?;
        Ok(())
    }

    /// Removes package
    pub async fn remove_package(&self, package_name: &str, direct: bool) -> Result<(), UhpmError> {
        remover::remove(package_name, &self.db, direct).await?;
        Ok(())
    }

    /// Removes specific package version
    pub async fn remove_package_version(
        &self,
        package_name: &str,
        version: &str,
        direct: bool,
    ) -> Result<(), UhpmError> {
        remover::remove_by_version(package_name, version, &self.db, direct).await?;
        Ok(())
    }

    /// Updates package to latest version
    pub async fn update_package(&self, package_name: &str, direct: bool) -> Result<(), UhpmError> {
        updater::update_package(package_name, &self.db, direct).await?;
        Ok(())
    }

    /// Switches package version
    pub async fn switch_version(
        &self,
        package_name: &str,
        version: Version,
        direct: bool,
    ) -> Result<(), UhpmError> {
        switcher::switch_version(package_name, version, &self.db, direct).await?;
        Ok(())
    }

    /// Lists installed packages
    pub async fn list_packages(&self) -> Result<Vec<(String, String, bool)>, UhpmError> {
        self.db.list_packages().await.map_err(UhpmError::from)
    }

    /// Loads repository configuration
    async fn load_repositories(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, UhpmError> {
        let repos_path = dirs::home_dir()
            .ok_or_else(|| {
                UhpmError::Config(ConfigError::NotFound(
                    "Home directory not found".to_string(),
                ))
            })?
            .join(".uhpm/repos.ron");

        parse_repos(&repos_path).map_err(|e| UhpmError::Repository(e.into()))
    }

    /// Caches repositories locally
    async fn cache_repos(repos: crate::repo::RepoMap) -> Vec<PathBuf> {
        cache_repo(repos).await
    }

    /// Gets repository database path
    fn get_repo_db_path(&self, repo_path: &str) -> Result<PathBuf, UhpmError> {
        let path = if let Some(stripped) = repo_path.strip_prefix("file://") {
            stripped
        } else {
            repo_path
        };
        Ok(PathBuf::from(path).join("repository.db"))
    }
}
