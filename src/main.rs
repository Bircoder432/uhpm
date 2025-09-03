use std::path::Path;

use uhpm::db;
use db::PackageDB;
use tracing::{info, warn, error};
use tracing_subscriber;
use clap::Parser;
use uhpm::cli::Cli;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let mut db_path = dirs::home_dir().unwrap();
    db_path.push(".uhpm");
    db_path.push("packages.db");
    info!("{:?}",db_path);
    let package_db = PackageDB::new(&db_path).await.unwrap();
    let args = Cli::parse();
    args.run(&package_db).await?;
    Ok(())
}
