use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::info;

// Используем модули из UHPM
use uhpm::package::{Package, meta_parser};
use uhpm::symlist;
use uhpm::clear_tmp; // если нужно очистить tmp при упаковке
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder;
use std::fs::File;

#[derive(Parser)]
#[command(name = "uhpmk", version, about = "Universal Home Package Maker")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Создать шаблон пакета (uhp.ron + symlist.ron)
    Init {
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
    },
    /// Упаковать пакет в .uhp
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
            info!("Шаблон uhp.ron создан в {}", out_path.display());

            let symlist_path = out_dir.join("symlist.ron");
            symlist::save_template(&symlist_path)?;
            info!("Шаблон symlist.ron создан в {}", symlist_path.display());
        }

        Commands::Pack { package_dir, out_dir } => {
            let out_dir = out_dir.unwrap_or(std::env::current_dir()?);


            let meta_path = package_dir.join("uhp.ron");
            if !meta_path.exists() {
                return Err(format!("Файл uhp.ron не найден в {}", package_dir.display()).into());
            }


            let pkg: Package = meta_parser(&meta_path)?;

            let filename = format!("{}-{}.uhp", pkg.name(), pkg.version());
            let archive_path = out_dir.join(filename);


            let tar_gz = File::create(&archive_path)?;
            let enc = GzEncoder::new(tar_gz, Compression::default());
            let mut tar = Builder::new(enc);

            tar.append_dir_all(".", &package_dir)?;
            tar.finish()?;

            info!("Пакет упакован в {}", archive_path.display());
        }
    }

    Ok(())
}
