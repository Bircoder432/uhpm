use crate::db::PackageDB;
use crate::error::{ConfigError, UhpmError};
use crate::package::{installer, remover, switcher, updater};
use crate::repo::{RepoDB, cache_repo, parse_repos};
use crate::{fetcher, repo};
use semver::Version;
use std::path::{Path, PathBuf};

pub struct PackageService {
    db: PackageDB,
}

impl PackageService {
    pub fn new(db: PackageDB) -> Self {
        Self { db }
    }

    pub async fn install_from_file(&self, path: &Path, direct: bool) -> Result<(), UhpmError> {
        installer::install(path, &self.db, direct).await?;
        Ok(())
    }

    pub async fn extract_package(&self, path: &Path) -> Result<(), UhpmError> {
        installer::unpack(path)?;
        Ok(())
    }

    pub async fn install_from_repo(
        &self,
        package_name: &str,
        version: Option<&str>,
        direct: bool,
    ) -> Result<(), UhpmError> {
        let repos = cache_repo(self.load_repositories().await.unwrap()).await;
        let mut urls_to_download = Vec::new();
        let mut found = false;

        for repo_path in &repos {
            if !repo_path.exists() {
                tracing::warn!(
                    "Repository database not found: {}",
                    repo_path.to_str().unwrap()
                );
                continue;
            }

            let repo_db = RepoDB::new(&repo_path).await?;
            let packages = repo_db.list_packages().await?;

            for (name, pkg_version, url) in packages {
                if name == package_name {
                    // Если версия не указана - берем первую найденную
                    // Если версия указана - проверяем совпадение
                    if version.is_none() || version.unwrap() == pkg_version {
                        urls_to_download.push(url);
                        found = true;

                        // Если версия указана явно, выходим после нахождения
                        if version.is_some() {
                            break;
                        }
                    }
                }
            }

            // Если нашли пакет и версия была указана явно, выходим из цикла по репозиториям
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

        // Добавим отладочную информацию
        tracing::info!("Found packages to download: {:?}", urls_to_download);

        fetcher::fetch_and_install_parallel(&urls_to_download, &self.db, direct).await?;
        Ok(())
    }

    pub async fn remove_package(&self, package_name: &str, direct: bool) -> Result<(), UhpmError> {
        remover::remove(package_name, &self.db, direct).await?;
        Ok(())
    }

    pub async fn remove_package_version(
        &self,
        package_name: &str,
        version: &str,
        direct: bool,
    ) -> Result<(), UhpmError> {
        remover::remove_by_version(package_name, version, &self.db, direct).await?;
        Ok(())
    }

    pub async fn update_package(&self, package_name: &str, direct: bool) -> Result<(), UhpmError> {
        updater::update_package(package_name, &self.db, direct).await?;
        Ok(())
    }

    pub async fn switch_version(
        &self,
        package_name: &str,
        version: Version,
        direct: bool,
    ) -> Result<(), UhpmError> {
        switcher::switch_version(package_name, version, &self.db, direct).await?;
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

    async fn cache_repos(repos: repo::RepoMap) -> Vec<PathBuf> {
        repo::cache_repo(repos).await
    }
    fn get_repo_db_path(&self, repo_path: &str) -> Result<PathBuf, UhpmError> {
        let path = if let Some(stripped) = repo_path.strip_prefix("file://") {
            stripped
        } else {
            repo_path
        };
        Ok(PathBuf::from(path).join("repository.db"))
    }
}
