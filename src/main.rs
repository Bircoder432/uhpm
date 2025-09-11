//! # UHPM Main Entry Point
//!
//! This is the executable entry point for **UHPM (Universal Home Package Manager)**.
//! It initializes logging, sets up the package database, and executes the CLI
//! commands provided by the user.
//!
//! ## Responsibilities
//! - Initialize tracing/logging via [`tracing_subscriber`].
//! - Locate and initialize the UHPM database at `~/.uhpm/packages.db`.
//! - Parse CLI arguments using [`clap`].
//! - Delegate execution to [`Cli::run`](uhpm::cli::Cli::run).
//!
//! ## Example
//! ```bash
//! # Install a package from a repository
//! uhpm install foo
//!
//! # Install from local file
//! uhpm install --file ./bar.uhp
//!
//! # List installed packages
//! uhpm list
//! ```

use clap::Parser;
use db::PackageDB;
use tracing::info;
use tracing_subscriber;
use uhpm::cli::Cli;
use uhpm::db;

/// Main entry point for UHPM.
///
/// - Initializes logging.
/// - Opens or creates the package database.
/// - Parses command-line arguments.
/// - Executes the requested command.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing/logging
    tracing_subscriber::fmt::init();

    // Determine database path (~/.uhpm/packages.db)
    let mut db_path = dirs::home_dir().unwrap();
    db_path.push(".uhpm");
    db_path.push("packages.db");
    info!("Using package database at {:?}", db_path);

    // Initialize database connection
    let package_db = PackageDB::new(&db_path)?.init().await?;

    // Parse CLI arguments and execute subcommand
    let args = Cli::parse();
    args.run(&package_db).await?;

    Ok(())
}
