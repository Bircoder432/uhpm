use clap::{Parser, Subcommand};
use std::path::PathBuf;
use uhpm::package::{Package, meta_parser};
use uhpm::symlist;
use uhpm::{error, info};

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::File;
use tar::Builder;

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

#[derive(Subcommand)]
enum Commands {
    Init {
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
    Pack {
        #[arg(short, long)]
        package_dir: PathBuf,
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            let pkg = Package::template();
            let out_path = out_dir.join("uhp.ron");
            pkg.save_to_ron(&out_path)?;
            info!("uhpmk.init.uhp_ron_created", out_path.display());

            let symlist_path = out_dir.join("symlist.ron");
            symlist::save_template(&symlist_path)?;
            info!("uhpmk.init.symlist_created", symlist_path.display());
        }

        Commands::Pack {
            package_dir,
            out_dir,
        } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);

            let meta_path = package_dir.join("uhp.ron");
            if !meta_path.exists() {
                error!("uhpmk.pack.meta_not_found", package_dir.display());
                return Err(format!("uhp.ron not found in {}", package_dir.display()).into());
            }

            let pkg: Package = meta_parser(&meta_path)?;

            let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
            let archive_path = out_dir.join(filename);

            let tar_gz = File::create(&archive_path)?;
            let enc = GzEncoder::new(tar_gz, Compression::default());
            let mut tar = Builder::new(enc);

            tar.append_dir_all(".", &package_dir)?;
            tar.finish()?;

            info!("uhpmk.pack.package_packed", archive_path.display());
        }
    }

    Ok(())
}
