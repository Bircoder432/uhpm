use crate::db::PackageDB;
use crate::error::{ConfigError, UhpmError};
use crate::fetcher;
use crate::package::{installer, remover, switcher, updater};
use crate::repo::{RepoDB, parse_repos};
use semver::Version;
use std::path::{Path, PathBuf};

pub struct PackageService {
    db: PackageDB,
}

impl PackageService {
    pub fn new(db: PackageDB) -> Self {
        Self { db }
    }

    pub async fn install_from_file(&self, path: &Path) -> Result<(), UhpmError> {
        installer::install(path, &self.db).await?;
        Ok(())
    }

    pub async fn install_from_repo(
        &self,
        package_name: &str,
        version: Option<&str>,
    ) -> Result<(), UhpmError> {
        let repos = self.load_repositories().await?;
        let mut urls_to_download = Vec::new();

        for (repo_name, repo_path) in &repos {
            let repo_db_path = self.get_repo_db_path(repo_path)?;
            if !repo_db_path.exists() {
                tracing::warn!("Repository database not found: {}", repo_name);
                continue;
            }

            let repo_db = RepoDB::new(&repo_db_path).await?;
            let packages = repo_db.list_packages().await?;

            for (name, pkg_version) in packages {
                if name == package_name {
                    if version.is_none() || version.unwrap() == pkg_version {
                        if let Ok(url) = repo_db.get_package(&name, &pkg_version).await {
                            urls_to_download.push(url);
                            break;
                        }
                    }
                }
            }
        }

        if urls_to_download.is_empty() {
            return Err(UhpmError::NotFound(format!(
                "Package {} not found in repositories",
                package_name
            )));
        }

        fetcher::fetch_and_install_parallel(&urls_to_download, &self.db).await?;
        Ok(())
    }

    pub async fn remove_package(&self, package_name: &str) -> Result<(), UhpmError> {
        remover::remove(package_name, &self.db).await?;
        Ok(())
    }

    pub async fn remove_package_version(
        &self,
        package_name: &str,
        version: &str,
    ) -> Result<(), UhpmError> {
        remover::remove_by_version(package_name, version, &self.db).await?;
        Ok(())
    }

    pub async fn update_package(&self, package_name: &str) -> Result<(), UhpmError> {
        updater::update_package(package_name, &self.db).await?;
        Ok(())
    }

    pub async fn switch_version(
        &self,
        package_name: &str,
        version: Version,
    ) -> Result<(), UhpmError> {
        switcher::switch_version(package_name, version, &self.db).await?;
        Ok(())
    }

    pub async fn list_packages(&self) -> Result<Vec<(String, String, bool)>, UhpmError> {
        self.db.list_packages().await.map_err(UhpmError::from)
    }

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

    fn get_repo_db_path(&self, repo_path: &str) -> Result<PathBuf, UhpmError> {
        let path = if let Some(stripped) = repo_path.strip_prefix("file://") {
            stripped
        } else {
            repo_path
        };
        Ok(PathBuf::from(path).join("packages.db"))
    }
}
