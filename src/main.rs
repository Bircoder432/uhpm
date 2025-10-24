//! # UHPM Main Entry Point
//!
//! This is the executable entry point for **UHPM (Universal Home Package Manager)**.
//! It initializes localized logging, sets up the package database, and executes
//! the CLI commands provided by the user.

use clap::Parser;
use dirs;
use uhpm::cli::Cli;
use uhpm::db::PackageDB;

// Import all macros from log.rs

use uhpm::{debug, info};

/// Main entry point for UHPM.
///
/// Responsibilities:
/// - Initialize localized tracing/logging.
/// - Open or create the package database.
/// - Parse CLI arguments.
/// - Execute the requested command.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing/logging
    tracing_subscriber::fmt::init();
    // Determine database path (~/.uhpm/packages.db)
    let mut db_path = dirs::home_dir().ok_or("Could not determine home directory")?;
    db_path.push(".uhpm");
    db_path.push("packages.db");

    // Log using localized info macro
    debug!("main.info.using_package_db");
    debug!("main.info.db_path_is", db_path.display()); // локализованный print

    // Initialize database connection
    let package_db = PackageDB::new(&db_path)?.init().await?;
    info!("main.info.uhpm_started");
    // Parse CLI arguments and execute subcommand
    let args = Cli::parse();
    args.run(&package_db).await?;

    // Optional: localized debug

    Ok(())
}
