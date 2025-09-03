use crate::db::PackageDB;
use crate::repo::{parse_repos, RepoDB, RepoError};
use crate::fetcher;
use tracing::{info, warn, error};
use semver::Version;

/// Ошибки обновления пакета
#[derive(thiserror::Error, Debug)]
pub enum UpdaterError {
    #[error("Package not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Repo error: {0}")]
    Repo(#[from] RepoError),

    #[error("DB error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Fetch error: {0}")]
    Fetch(#[from] crate::fetcher::FetchError),
}

/// Проверяет и обновляет пакет
pub async fn update_package(pkg_name: &str, package_db: &PackageDB) -> Result<(), UpdaterError> {
    // Проверяем, установлен ли пакет
    let installed_version = package_db.get_package_version(pkg_name).await?;
    if installed_version.is_none() {
        warn!("Пакет {} не установлен", pkg_name);
        return Err(UpdaterError::NotFound(pkg_name.to_string()));
    }

    let installed_version = installed_version.unwrap();
    info!("Установленная версия {}: {}", pkg_name, installed_version);

    // Парсим репозитории
    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
    let repos = parse_repos(&repos_path)?;

    let mut latest_url = None;
    let mut latest_version: Option<Version> = None;

    for (repo_name, repo_path) in repos {
        let repo_path = repo_path.strip_prefix("file://").unwrap_or(&repo_path).to_string();
        let repo_db_path = std::path::Path::new(&repo_path).join("packages.db");

        if !repo_db_path.exists() {
            warn!("База репозитория {} не найдена, пропускаю", repo_name);
            continue;
        }

        let repo_db = RepoDB::new(&repo_db_path).await?;
        let pkg_list = repo_db.list_packages().await?;

        for (name, ver_str) in pkg_list {
            if name == pkg_name {
                let ver = Version::parse(&ver_str).unwrap_or(Version::new(0,0,0));
                let inst_ver = Version::parse(&installed_version).unwrap_or(Version::new(0,0,0));
                if ver > inst_ver {
                    latest_version = Some(ver);
                    latest_url = Some(repo_db.get_package(&name, &ver_str).await?);
                }
            }
        }
    }

    if let Some(url) = latest_url {
        info!("Найдена новая версия пакета {}: {}", pkg_name, latest_version.unwrap());
        // Скачиваем и устанавливаем новую версию
        let downloaded = fetcher::fetch_and_install_parallel(&[url], package_db).await?;
        info!("Пакет {} обновлён", pkg_name);
    } else {
        info!("Пакет {} актуален, обновлений нет", pkg_name);
    }

    Ok(())
}
