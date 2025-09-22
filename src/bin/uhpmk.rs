//! # UHPM Package Maker (uhpmk)
//!
//! This binary crate provides a command-line utility for creating and packaging UHPM packages.
//! It offers three main subcommands: `init` for initializing new package templates, `build` for
//! building packages using build scripts, and `pack` for creating compressed package archives.
//!
//! ## Subcommands
//!
//! - `init`: Creates template package metadata (`uhp.ron`) and symlink list (`symlist.ron`) files
//! - `build`: Executes package build script (`uhpbuild`) and optionally packages the result
//! - `pack`: Packages a directory containing package files into a compressed `.uhp` archive
//!
//! ## Usage Examples
//!
//! Initialize a new package template:
//! ```bash
//! uhpmk init --out-dir ./my_package
//! ```
//!
//! Build a package using its build script:
//! ```bash
//! uhpmk build --package-dir ./my_package
//! ```
//!
//! Build and immediately package:
//! ```bash
//! uhpmk build --package-dir ./my_package --make
//! ```
//!
//! Package an existing package directory:
//! ```bash
//! uhpmk pack --package-dir ./my_package --out-dir ./dist
//! ```

use clap::{Parser, Subcommand};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use tar::Builder;
use uhpm::error::PackerError;
use uhpm::package::{Package, meta_parser};
use uhpm::symlist;
use uhpm::{error, info};

/// CLI interface for UHPM Package Maker
#[derive(Parser)]
#[command(
    name = "uhpmk",
    version = "1.0",
    about = "Universal Home Package Maker",
    long_about = "A utility for creating and packaging UHPM (Universal Home Package Manager) packages.\n\nProvides commands to initialize package templates, build packages using build scripts, and create compressed package archives."
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

    /// Build a package using its build script
    ///
    /// Executes the uhpbuild shell script in the package directory to build the package.
    /// Optionally creates a .uhp archive after successful build.
    Build {
        /// Directory containing package files and build script
        #[arg(value_name = "PATH")]
        package_dir: PathBuf,

        /// Immediately package the built result into a .uhp archive
        #[arg(short, long)]
        pack: bool,

        #[arg(short, long)]
        install: bool,

        /// Output directory for the created package archive (only used with --make)
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },

    /// Package a directory into a .uhp archive
    ///
    /// Compresses the specified package directory into a .uhp archive file
    /// containing all package files and metadata.
    Pack {
        /// Directory containing package files to archive
        #[arg(value_name = "PATH")]
        package_dir: PathBuf,
        /// Output directory for the created package archive
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

/// Main entry point for uhpmk utility
///
/// Parses command line arguments and executes the appropriate subcommand
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

        Commands::Build {
            package_dir,
            pack,
            install,
            out_dir,
        } => {
            // Verify package directory exists
            if !package_dir.exists() {
                error!("uhpmk.build.dir_not_found", package_dir.display());
                return Err(
                    format!("Package directory not found: {}", package_dir.display()).into(),
                );
            }

            // Verify build script exists
            let build_script_path = package_dir.join("uhpbuild");
            if !build_script_path.exists() {
                error!("uhpmk.build.script_not_found", build_script_path.display());
                return Err(
                    format!("Build script not found: {}", build_script_path.display()).into(),
                );
            }

            // Make build script executable (Unix-like systems)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = std::fs::metadata(&build_script_path)?.permissions();
                perms.set_mode(0o755); // rwxr-xr-x
                std::fs::set_permissions(&build_script_path, perms)?;
            }

            info!("uhpmk.build.executing_script", build_script_path.display());

            // Execute the build script
            let status = Command::new(&build_script_path)
                .current_dir(&package_dir)
                .status()?;

            if !status.success() {
                error!("uhpmk.build.script_failed", status.code().unwrap_or(-1));
                return Err("Build script execution failed".into());
            }

            info!("uhpmk.build.script_completed");

            // If --make flag is set, package the result
            if pack {
                let out_dir = out_dir.unwrap_or(std::env::current_dir()?);
                let pkg_path = packer(package_dir.join("package"), out_dir)?;
                if install {
                    let mut db_path =
                        dirs::home_dir().ok_or("Could not determine home directory")?;
                    db_path.push(".uhpm");
                    db_path.push("packages.db");
                    let package_db = uhpm::db::PackageDB::new(&db_path)?.init().await.unwrap();
                    uhpm::package::installer::install(&pkg_path, &package_db)
                        .await
                        .unwrap();
                }
            }
        }

        Commands::Pack {
            package_dir,
            out_dir,
        } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            packer(package_dir, out_dir);
        }
    }

    Ok(())
}

fn packer(package_dir: PathBuf, out_dir: PathBuf) -> Result<PathBuf, PackerError> {
    // let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

    // Verify package metadata exists
    let meta_path = package_dir.join("uhp.ron");
    if !meta_path.exists() {
        error!("uhpmk.pack.meta_not_found", package_dir.display());
        // return Err(format!("uhp.ron not found in {}", package_dir.display()).into());
    }

    // Parse package metadata
    let pkg: Package = meta_parser(&meta_path).unwrap();

    // Create package archive
    let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
    let archive_path = out_dir.join(filename);

    let tar_gz = File::create(&archive_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);

    // Add all files from package directory to archive
    tar.append_dir_all(".", &package_dir).unwrap();
    tar.finish().unwrap();

    return Ok(archive_path);
    info!("uhpmk.pack.package_packed", archive_path.display());
}
