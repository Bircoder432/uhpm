use crate::service::PackageService;
use crate::{error, info, lprintln};
use clap::CommandFactory;
use clap::{Parser, Subcommand};
use clap_complete::{
    generate,
    shells::{Bash, Fish, Zsh},
};
use std::io;
use std::path::PathBuf;

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
    Update {
        #[arg(short, long)]
        file: Option<PathBuf>,
        #[arg(value_name = "PACKAGE")]
        packages: Vec<String>,
    },
    Switch {
        #[arg(value_name = "PACKAGE@VERSION")]
        target: String,
    },
    Completions {
        shell: String,
    },
}

impl Cli {
    pub async fn run(&self, service: &PackageService) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            Commands::Install {
                file,
                package,
                version,
            } => {
                if let Some(path) = file {
                    info!("cli.install.from_file", path.display());
                    service.install_from_file(path).await?;
                } else if !package.is_empty() {
                    for pkg_name in package {
                        info!("cli.install.from_repo", pkg_name);
                        service
                            .install_from_repo(pkg_name, version.as_deref())
                            .await?;
                    }
                } else {
                    error!("cli.install.no_file_or_package");
                }
            }

            Commands::Remove { packages } => {
                if packages.is_empty() {
                    error!("cli.remove.no_packages");
                } else {
                    for pkg_name in packages {
                        if pkg_name.contains('@') {
                            let parts: Vec<&str> = pkg_name.split('@').collect();
                            if parts.len() == 2 {
                                let (pkg_name, pkg_version) = (parts[0], parts[1]);
                                info!("cli.remove.parts", pkg_name, pkg_version);
                                service
                                    .remove_package_version(pkg_name, pkg_version)
                                    .await?;
                            } else {
                                error!("cli.remove.invalid_format", pkg_name);
                            }
                        } else {
                            info!("cli.remove.removing", pkg_name);
                            service.remove_package(pkg_name).await?;
                        }
                    }
                }
            }

            Commands::List => {
                let packages = service.list_packages().await?;
                if packages.is_empty() {
                    lprintln!("cli.list.no_packages");
                } else {
                    lprintln!("cli.list.installed_packages");
                    for (name, version, current) in packages {
                        let marker = if current { '*' } else { ' ' };
                        lprintln!("cli.list.package_format", name, version, marker);
                    }
                }
            }

            Commands::Update { file, packages } => {
                if let Some(path) = file {
                    info!("cli.update.from_file", path.display());
                    service.install_from_file(path).await?;
                } else {
                    for package in packages {
                        match service.update_package(package).await {
                            Ok(()) => info!("cli.update.success", package),
                            Err(e) => error!("cli.update.error", package, e),
                        }
                    }
                }
            }

            Commands::Switch { target } => {
                let parts: Vec<&str> = target.split('@').collect();
                if parts.len() != 2 {
                    error!("cli.switch.invalid_format", target);
                    return Ok(());
                }

                let pkg_name = parts[0];
                let pkg_version = parts[1];

                match semver::Version::parse(pkg_version) {
                    Ok(version) => {
                        info!("cli.switch.switching", pkg_name, pkg_version);
                        service.switch_version(pkg_name, version).await?;
                        info!("cli.switch.success", pkg_name, pkg_version);
                    }
                    Err(e) => {
                        error!("cli.switch.invalid_version", pkg_version, e);
                    }
                }
            }

            Commands::Completions { shell } => match shell.to_lowercase().as_str() {
                "bash" => generate(Bash, &mut Cli::command(), "uhpm", &mut io::stdout()),
                "zsh" => generate(Zsh, &mut Cli::command(), "uhpm", &mut io::stdout()),
                "fish" => generate(Fish, &mut Cli::command(), "uhpm", &mut io::stdout()),
                other => println!("Unsupported shell: {}", other),
            },
        }

        Ok(())
    }
}
