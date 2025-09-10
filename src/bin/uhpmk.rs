//! # uhpmk
//!
//! `uhpmk` (Universal Home Package Maker) is a companion utility for UHPM.
//! It provides tools to initialize new packages and pack them into `.uhp` archives.
//!
//! ## Subcommands
//! - `init`: Generates template files (`uhp.ron` and `symlist.ron`) in the target directory.
//! - `pack`: Creates a `.uhp` archive from a package directory containing `uhp.ron`.

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use tar::Builder;

use uhpm::package::{Package, meta_parser};
use uhpm::symlist;

/// Command-line interface for `uhpmk`
#[derive(Parser)]
#[command(name = "uhpmk", version, about = "Universal Home Package Maker")]
struct Cli {
    /// Available subcommands
    #[command(subcommand)]
    command: Commands,
}

/// Subcommands supported by `uhpmk`
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new package by generating `uhp.ron` and `symlist.ron`
    Init {
        /// Output directory for generated files (defaults to current directory)
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },

    /// Pack a package directory into a `.uhp` archive
    Pack {
        /// Path to the package directory containing `uhp.ron`
        #[arg(short, long)]
        package_dir: PathBuf,

        /// Output directory for the archive (defaults to current directory)
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

/// Entry point for `uhpmk`
///
/// This function parses CLI arguments, executes the selected subcommand,
/// and logs relevant information.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            // Create a template package metadata file
            let pkg = Package::template();
            let out_path = out_dir.join("uhp.ron");
            pkg.save_to_ron(&out_path)?;
            info!("Template uhp.ron created at {}", out_path.display());

            // Create a template symlist file
            let symlist_path = out_dir.join("symlist.ron");
            symlist::save_template(&symlist_path)?;
            info!("Template symlist.ron created at {}", symlist_path.display());
        }

        Commands::Pack { package_dir, out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            // Ensure that package metadata exists
            let meta_path = package_dir.join("uhp.ron");
            if !meta_path.exists() {
                return Err(format!("uhp.ron not found in {}", package_dir.display()).into());
            }

            // Parse package metadata
            let pkg: Package = meta_parser(&meta_path)?;

            // Create archive name based on package name and version
            let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
            let archive_path = out_dir.join(filename);

            // Build `.uhp` (tar.gz) archive
            let tar_gz = File::create(&archive_path)?;
            let enc = GzEncoder::new(tar_gz, Compression::default());
            let mut tar = Builder::new(enc);

            tar.append_dir_all(".", &package_dir)?;
            tar.finish()?;

            info!("Package packed into {}", archive_path.display());
        }
    }

    Ok(())
}
