use crate::db::PackageDB;
use crate::fetcher;
use crate::package::installer;
use crate::package::remover;
use crate::package::switcher;
use crate::package::updater;
use crate::repo::{RepoDB, parse_repos};
use crate::self_remove;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};


#[derive(Parser)]
#[command(name = "uhpm", version, about = "Universal Home Package Manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}


#[derive(Subcommand)]
pub enum Commands {

    Install {

        #[arg(short, long)]
        file: Option<PathBuf>,

        #[arg(value_name = "PACKAGE")]
        package: Vec<String>,

        #[arg(short, long)]
        version: Option<String>,
    },


    Remove {

        #[arg(value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    List,
    SelfRemove,

    Update {

        package: String,
    },

    Switch {

        #[arg(value_name = "PACKAGE@VERSION")]
        target: String,
    },
}

impl Cli {
    pub async fn run(&self, db: &PackageDB) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Install {
                file,
                package,
                version,
            } => {

                if let Some(path) = file {
                    info!("Устанавливаю пакет из файла: {}", path.display());
                    installer::install(path, db).await.unwrap();
                    return Ok(());
                }


                if !package.is_empty() {
                    let repos_path = dirs::home_dir().unwrap().join(".uhpm/repos.ron");
                    let repos = parse_repos(&repos_path)?;

                    for pkg_name in package {
                        let mut urls_to_download = Vec::new();
                        for (repo_name, repo_path) in &repos {
                            let repo_path =
                                if let Some(stripped) = repo_path.strip_prefix("file://") {
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
                                    if version.is_none()
                                        || version.as_ref().unwrap() == &pkg_version
                                    {
                                        if let Ok(url) =
                                            repo_db.get_package(&name, &pkg_version).await
                                        {
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
                    for (name, version, current) in packages {
                        let chr = if current { '*' } else { ' ' };
                        println!(" - {} {} {}", name, version, chr);
                    }
                }
            }
            Commands::Update { package } => {
                let updater_result = updater::update_package(package, db).await;
                match updater_result {
                    Ok(_) => info!("Пакет '{}' обновлён или уже актуален", package),
                    Err(updater::UpdaterError::NotFound(_)) => {
                        println!("Пакет '{}' не установлен", package);
                    }
                    Err(e) => {
                        error!("Ошибка при обновлении пакета '{}': {:?}", package, e);
                    }
                }
            }
            Commands::Switch { target } => {

                let parts: Vec<&str> = target.split('@').collect();
                if parts.len() != 2 {
                    error!("Неверный формат '{}'. Используй: имя@версия", target);
                    return Ok(());
                }

                let pkg_name = parts[0];
                let pkg_version = parts[1];


                match semver::Version::parse(pkg_version) {
                    Ok(ver) => {

                        info!(
                            "Переключаю пакет '{}' на версию {}...",
                            pkg_name, pkg_version
                        );
                        match switcher::switch_version(pkg_name, ver, db).await {
                            Ok(_) => {
                                info!("Пакет '{}' успешно переключён на {}", pkg_name, pkg_version)
                            }
                            Err(e) => error!("Ошибка при переключении: {:?}", e),
                        }
                    }
                    Err(e) => {
                        error!("Неверный формат версии '{}': {}", pkg_version, e);
                    }
                }
            }
            Commands::SelfRemove => {
                self_remove::self_remove()?;
            }
        }

        Ok(())
    }
}
