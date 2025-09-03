use clap::{Parser, Subcommand};
use std::path::{PathBuf, Path};
use tracing::{info, error, warn};

use crate::db::PackageDB;
use crate::installer;
use crate::remover;
use crate::repo::{RepoDB, parse_repos, RepoError};
use crate::fetcher;

/// Основная структура CLI
#[derive(Parser)]
#[command(name = "uhpm", version, about = "Universal Home Package Manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Подкоманды
#[derive(Subcommand)]
pub enum Commands {
    /// Установить пакет
    Install {
            /// Установить пакет из файла (.uhp)
            #[arg(short, long)]
            file: Option<PathBuf>,
            /// Установить пакеты из репозитория (по имени)
            #[arg(value_name = "PACKAGE")]
            package: Vec<String>,
            /// Версия пакета (если не указана, берется последняя)
            #[arg(short, long)]
            version: Option<String>,
        },


    /// Удалить пакет
    Remove {
        /// Имя пакета
        #[arg(value_name = "PACKAGE")]
            packages: Vec<String>,
    },

    List,

    Update {
            /// Имя пакета для проверки обновлений
            package: String,
        },

}

impl Cli {
    pub async fn run(&self, db: &PackageDB) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Install { file, package, version } => {
                // Установка из файла (file)
                if let Some(path) = file {
                    info!("Устанавливаю пакет из файла: {}", path.display());
                    installer::install(path, db).await.unwrap();
                    return Ok(());
                }

                // Установка из репозитория
                if !package.is_empty() {
                    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
                    let repos = parse_repos(&repos_path)?;

                    for pkg_name in package {
                        let mut urls_to_download = Vec::new();
                        for (repo_name, repo_path) in &repos {
                            let repo_path = if let Some(stripped) = repo_path.strip_prefix("file://") {
                                stripped.to_string()
                            } else {
                                repo_path.clone()
                            };

                            let repo_db_path = Path::new(&repo_path).join("packages.db");
                            if !repo_db_path.exists() {
                                warn!("База репозитория {} не найдена, пропускаю", repo_name);
                                continue;
                            }

                            let repo_db = RepoDB::new(&repo_db_path).await?;
                            let pkg_list = repo_db.list_packages().await?;

                            for (name, pkg_version) in pkg_list {
                                if name == *pkg_name {
                                    if version.is_none() || version.as_ref().unwrap() == &pkg_version {
                                        if let Ok(url) = repo_db.get_package(&name, &pkg_version).await {
                                            urls_to_download.push(url);
                                        }
                                    }
                                }
                            }
                        }

                        if urls_to_download.is_empty() {
                            error!("Пакет {} не найден ни в одном репозитории", pkg_name);
                        } else {
                            info!("Скачиваю и устанавливаю пакет {}...", pkg_name);
                            fetcher::fetch_and_install_parallel(&urls_to_download, db).await?;
                        }
                    }
                } else {
                    error!("Не указан ни файл, ни имя пакета для установки");
                }
            }

            Commands::Remove { packages } => {
                if packages.is_empty() {
                    error!("Не указаны пакеты для удаления");
                } else {
                    for pkg_name in packages {
                        info!("Удаляю пакет: {}", pkg_name);
                        if let Err(e) = remover::remove(pkg_name, db).await {
                            error!("Ошибка при удалении {}: {:?}", pkg_name, e);
                        }
                    }
                }
            }

            Commands::List => {
                let packages = db.list_packages().await?;
                if packages.is_empty() {
                    println!("Установленных пакетов нет");
                } else {
                    println!("Установленные пакеты:");
                    for (name, version) in packages {
                        println!(" - {} {}", name, version);
                    }
                }
            }
            Commands::Update { package } => {
                let updater_result = crate::updater::update_package(package, db).await;
                match updater_result {
                    Ok(_) => info!("Пакет '{}' обновлён или уже актуален", package),
                    Err(crate::updater::UpdaterError::NotFound(_)) => {
                        println!("Пакет '{}' не установлен", package);
                    }
                    Err(e) => {
                        error!("Ошибка при обновлении пакета '{}': {:?}", package, e);
                    }
                }
            }
        }

        Ok(())
    }
}
