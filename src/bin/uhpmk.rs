//! # UHPM Package Maker (uhpmk)
//!
//! This binary crate provides a command-line utility for creating and packaging UHPM packages.

use clap::{Parser, Subcommand};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use tar::Builder;
use uhpm::package::{Package, meta_parser};
use uhpm::symlist;
use uhpm::{error, info};

/// CLI interface for UHPM Package Maker
#[derive(Parser)]
#[command(
    name = "uhpmk",
    version = "1.0",
    about = "Universal Home Package Maker"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for uhpmk
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new package template
    Init {
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },

    /// Build a package using its build script
    Build {
        #[arg(value_name = "PATH")]
        package_dir: PathBuf,
        #[arg(short, long)]
        pack: bool,
        #[arg(short, long)]
        install: bool,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },

    /// Package a directory into a .uhp archive
    Pack {
        #[arg(value_name = "PATH")]
        package_dir: PathBuf,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

#[derive(thiserror::Error, Debug)]
enum PackerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Package error: {0}")]
    Package(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            let pkg = Package::template();
            let out_path = out_dir.join("uhp.toml");
            pkg.save_to_toml(&out_path)?;
            info!("uhpmk.init.uhp_toml_created", out_path.display());

            let symlist_path = out_dir.join("symlist");
            symlist::save_template(&symlist_path)?;
            info!("uhpmk.init.symlist_created", symlist_path.display());
        }

        Commands::Build {
            package_dir,
            pack,
            install,
            out_dir,
        } => {
            if !package_dir.exists() {
                error!("uhpmk.build.dir_not_found", package_dir.display());
                return Err("Package directory not found".into());
            }

            let build_script_path = package_dir.join("uhpbuild");
            if !build_script_path.exists() {
                error!("uhpmk.build.script_not_found", build_script_path.display());
                return Err("Build script not found".into());
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&build_script_path)?.permissions();
                perms.set_mode(0o755);
                std::fs::set_permissions(&build_script_path, perms)?;
            }

            info!("uhpmk.build.executing_script", build_script_path.display());

            let status = Command::new(&build_script_path)
                .current_dir(&package_dir)
                .status()?;

            if !status.success() {
                error!("uhpmk.build.script_failed", status.code().unwrap_or(-1));
                return Err("Build script execution failed".into());
            }

            info!("uhpmk.build.script_completed");

            if pack {
                let out_dir = out_dir.unwrap_or(std::env::current_dir()?);
                let pkg_path = packer(package_dir.join("package"), out_dir)?;
                if install {
                    let mut db_path =
                        dirs::home_dir().ok_or("Could not determine home directory")?;
                    db_path.push(".uhpm");
                    db_path.push("packages.db");
                    let package_db = uhpm::db::PackageDB::new(&db_path)?.init().await?;
                    uhpm::package::installer::install(&pkg_path, &package_db).await?;
                }
            }
        }

        Commands::Pack {
            package_dir,
            out_dir,
        } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);
            packer(package_dir, out_dir)?;
        }
    }

    Ok(())
}

fn packer(package_dir: PathBuf, out_dir: PathBuf) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let meta_path = package_dir.join("uhp.toml");
    if !meta_path.exists() {
        error!("uhpmk.pack.meta_not_found", meta_path.display());
        return Err(PackerError::Package(format!(
            "uhp.toml not found in {}",
            package_dir.display()
        ))
        .into());
    }

    let pkg: Package = meta_parser(&meta_path)
        .map_err(|e| PackerError::Package(format!("Failed to parse package metadata: {}", e)))?;

    let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
    let archive_path = out_dir.join(&filename);

    let tar_gz = File::create(&archive_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    tar.append_dir_all(".", &package_dir)?;
    tar.finish()?;

    info!("uhpmk.pack.package_packed", archive_path.display());
    Ok(archive_path)
}
