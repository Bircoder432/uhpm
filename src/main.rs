use clap::Parser;
use dirs;
use uhpm::cli::Cli;
use uhpm::db::PackageDB;
use uhpm::service::PackageService;
use uhpm::{debug, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let mut db_path = dirs::home_dir().ok_or("Could not determine home directory")?;
    db_path.push(".uhpm");
    db_path.push("packages.db");

    debug!("main.info.using_package_db");
    debug!("main.info.db_path_is", db_path.display());

    let package_db = PackageDB::new(&db_path)?.init().await?;
    let package_service = PackageService::new(package_db);

    info!("main.info.uhpm_started");

    let args = Cli::parse();
    args.run(&package_service).await?;

    Ok(())
}
