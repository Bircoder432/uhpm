//! # Command-Line Interface (CLI)
//!
//! This module defines the CLI for **UHPM (Universal Home Package Manager)**.
//! It provides user-facing commands for managing packages, including
//! installation, removal, listing, updates, switching versions, and
//! self-removal of the package manager itself.
//!
//! ## Responsibilities
//! - Parse CLI arguments using [`clap`].
//! - Provide subcommands for common package operations.
//! - Delegate logic to corresponding modules (`installer`, `remover`, `updater`, `switcher`, etc.).
//!
//! ## Commands
//! - **Install**: Install a package from a file or repository.
//! - **Remove**: Uninstall one or more packages.
//! - **List**: Show all installed packages, with the current version marked.
//! - **Update**: Update a package to the latest available version.
//! - **Switch**: Switch to a specific installed version of a package.
//! - **SelfRemove**: Uninstall UHPM itself.

use crate::db::PackageDB;
use crate::fetcher;
use crate::package::installer;
use crate::package::remover;
use crate::package::switcher;
use crate::package::updater;
use crate::repo::{RepoDB, parse_repos};
use crate::self_remove;
use clap::CommandFactory;
use clap::{Parser, Subcommand};
use clap_complete::{
    generate,
    shells::{Bash, Fish, Zsh},
};
use std::io;
use std::path::{Path, PathBuf};
use tracing::{error, info, warn};

/// Main CLI parser for UHPM.
///
/// Defines the CLI interface with global options and subcommands.
/// Built on top of [`clap::Parser`].
#[derive(Parser)]
#[command(name = "uhpm", version, about = "Universal Home Package Manager")]
pub struct Cli {
    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Commands,
}

/// Available UHPM subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Install a package from file or repository.
    Install {
        /// Install a package from a `.uhp` file.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Install packages from repositories by name.
        #[arg(value_name = "PACKAGE")]
        package: Vec<String>,

        /// Specify a version (if omitted, installs the latest).
        #[arg(short, long)]
        version: Option<String>,
    },

    /// Remove installed packages.
    Remove {
        /// Names of packages to remove.
        #[arg(value_name = "PACKAGE")]
        packages: Vec<String>,
    },

    /// List installed packages.
    ///
    /// Displays all installed packages and their versions.
    /// The currently active version is marked with `*`.
    List,

    /// Remove UHPM itself.
    SelfRemove,

    /// Update a package to the latest available version.
    Update {
        /// Name of the package to update.
        package: String,
    },

    /// Switch active version of a package.
    ///
    /// Format: `name@version`
    Switch {
        /// Target package and version, e.g. `foo@1.2.3`.
        #[arg(value_name = "PACKAGE@VERSION")]
        target: String,
    },

    /// Generate shell completion scripts
    Completions {
        /// Target shell (e.g. fish,bash,zsh)
        shell: String,
    },
}

impl Cli {
    /// Run the CLI, executing the chosen subcommand.
    ///
    /// # Arguments
    /// - `db`: Reference to the package database.
    ///
    /// # Errors
    /// Returns a boxed error if any operation fails.
    pub async fn run(&self, db: &PackageDB) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Install {
                file,
                package,
                version,
            } => {
                // Install from file
                if let Some(path) = file {
                    info!("Installing package from file: {}", path.display());
                    installer::install(path, db).await.unwrap();
                    return Ok(());
                }

                // Install from repository
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
                                warn!("Repository database {} not found, skipping", repo_name);
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
                            error!("Package {} not found in any repository", pkg_name);
                        } else {
                            info!("Downloading and installing package {}...", pkg_name);
                            fetcher::fetch_and_install_parallel(&urls_to_download, db).await?;
                        }
                    }
                } else {
                    error!("Neither file nor package name specified for installation");
                }
            }

            Commands::Remove { packages } => {
                if packages.is_empty() {
                    error!("No packages specified for removal");
                } else {
                    for pkg_name in packages {
                        info!("Removing package: {}", pkg_name);
                        if let Err(e) = remover::remove(pkg_name, db).await {
                            error!("Failed to remove {}: {:?}", pkg_name, e);
                        }
                    }
                }
            }

            Commands::List => {
                let packages = db.list_packages().await?;
                if packages.is_empty() {
                    println!("No installed packages");
                } else {
                    println!("Installed packages:");
                    for (name, version, current) in packages {
                        let chr = if current { '*' } else { ' ' };
                        println!(" - {} {} {}", name, version, chr);
                    }
                }
            }

            Commands::Update { package } => {
                let updater_result = updater::update_package(package, db).await;
                match updater_result {
                    Ok(_) => info!("Package '{}' updated or already up to date", package),
                    Err(updater::UpdaterError::NotFound(_)) => {
                        println!("Package '{}' is not installed", package);
                    }
                    Err(e) => {
                        error!("Error updating package '{}': {:?}", package, e);
                    }
                }
            }

            Commands::Switch { target } => {
                let parts: Vec<&str> = target.split('@').collect();
                if parts.len() != 2 {
                    error!("Invalid format '{}'. Use: name@version", target);
                    return Ok(());
                }

                let pkg_name = parts[0];
                let pkg_version = parts[1];

                match semver::Version::parse(pkg_version) {
                    Ok(ver) => {
                        info!(
                            "Switching package '{}' to version {}...",
                            pkg_name, pkg_version
                        );
                        match switcher::switch_version(pkg_name, ver, db).await {
                            Ok(_) => {
                                info!(
                                    "Package '{}' successfully switched to {}",
                                    pkg_name, pkg_version
                                )
                            }
                            Err(e) => error!("Error switching version: {:?}", e),
                        }
                    }
                    Err(e) => {
                        error!("Invalid version format '{}': {}", pkg_version, e);
                    }
                }
            }

            Commands::SelfRemove => {
                self_remove::self_remove()?;
            }
            Commands::Completions { shell } => {
                match shell.to_lowercase().as_str() {
                    "bash" => generate(Bash, &mut Cli::command(), "uhpm", &mut io::stdout()),
                    "zsh" => generate(Zsh, &mut Cli::command(), "uhpm", &mut io::stdout()),
                    "fish" => generate(Fish, &mut Cli::command(), "uhpm", &mut io::stdout()),
                    other => {
                        println!("Unsupported shell: {}", other);
                    }
                }
                return Ok(());
            }
        }

        Ok(())
    }
}
