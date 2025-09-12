//! # UHPM Package Maker (uhpmk)
//!
//! This binary crate provides a command-line utility for creating and packaging UHPM packages.
//! It offers two main subcommands: `init` for initializing new package templates and `pack` for
//! creating compressed package archives.
//!
//! ## Subcommands
//!
//! - `init`: Creates template package metadata (`uhp.ron`) and symlink list (`symlist.ron`) files
//! - `pack`: Packages a directory containing package files into a compressed `.uhp` archive
//!
//! ## Usage Examples
//!
//! Initialize a new package template:
//! ```bash
//! uhpmk init --out-dir ./my_package
//! ```
//!
//! Package an existing package directory:
//! ```bash
//! uhpmk pack --package-dir ./my_package --out-dir ./dist
//! ```

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use uhpm::package::{Package, meta_parser};
use uhpm::symlist;
use uhpm::{error, info};

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use tar::Builder;

/// CLI interface for UHPM Package Maker
#[derive(Parser)]
#[command(
    name = "uhpmk",
    version = "1.0",
    about = "Universal Home Package Maker",
    long_about = "A utility for creating and packaging UHPM (Universal Home Package Manager) packages.\n\nProvides commands to initialize package templates and create compressed package archives."
)]
struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for uhpmk
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new package template
    ///
    /// Creates template files for package metadata (uhp.ron) and symbolic link definitions (symlist.ron)
    /// in the specified output directory or current working directory if not specified.
    Init {
        /// Output directory for template files
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
    /// Package a directory into a .uhp archive
    ///
    /// Compresses the specified package directory into a .uhp archive file
    /// containing all package files and metadata.
    Pack {
        /// Directory containing package files to archive
        #[arg(short, long)]
        package_dir: PathBuf,
        /// Output directory for the created package archive
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

/// Main entry point for uhpmk utility
///
/// Parses command line arguments and executes the appropriate subcommand
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            // Create package metadata template
            let pkg = Package::template();
            let out_path = out_dir.join("uhp.ron");
            pkg.save_to_ron(&out_path)?;
            info!("uhpmk.init.uhp_ron_created", out_path.display());

            // Create symlink list template
            let symlist_path = out_dir.join("symlist.ron");
            symlist::save_template(&symlist_path)?;
            info!("uhpmk.init.symlist_created", symlist_path.display());
        }

        Commands::Pack {
            package_dir,
            out_dir,
        } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            // Verify package metadata exists
            let meta_path = package_dir.join("uhp.ron");
            if !meta_path.exists() {
                error!("uhpmk.pack.meta_not_found", package_dir.display());
                return Err(format!("uhp.ron not found in {}", package_dir.display()).into());
            }

            // Parse package metadata
            let pkg: Package = meta_parser(&meta_path)?;

            // Create package archive
            let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
            let archive_path = out_dir.join(filename);

            let tar_gz = File::create(&archive_path)?;
            let enc = GzEncoder::new(tar_gz, Compression::default());
            let mut tar = Builder::new(enc);

            // Add all files from package directory to archive
            tar.append_dir_all(".", &package_dir)?;
            tar.finish()?;

            info!("uhpmk.pack.package_packed", archive_path.display());
        }
    }

    Ok(())
}
