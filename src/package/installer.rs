use crate::db::PackageDB;
use crate::package::{Package, Source};
use crate::symlist;
use semver::Version;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Meta(crate::package::MetaParseError),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

impl From<crate::package::MetaParseError> for InstallError {
    fn from(e: crate::package::MetaParseError) -> Self {
        InstallError::Meta(e)
    }
}

pub async fn install(pkg_path: &Path, db: &PackageDB) -> Result<(), InstallError> {
    let unpacked = unpack(pkg_path)?;
    info!("Package unpacked to {:?}", unpacked);

    let meta_path = unpacked.join("uhp.ron");
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!("Parsed package meta: {:?}", package_meta);
    let pkg_name: &str = package_meta.name();
    let version: &Version = package_meta.version();
    if let Some(installed_version) = db.is_installed(pkg_name).await.unwrap() {
        if installed_version >= *version {
            info!(
                "Пакет {} версии {} уже установлен, пропускаем",
                pkg_name, installed_version
            );
            return Ok(());
        }
    }
    let package_root = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!(
            "{}-{}",
            package_meta.name(),
            package_meta.version()
        ));
    if package_root.exists() {
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;

    fs::rename(&unpacked, &package_root)?;

    let mut installed_files = Vec::new();

    // let bin_dir = package_root.join("bin");
    // if bin_dir.exists() {
    //     for entry in fs::read_dir(&bin_dir)? {
    //         let entry = entry?;
    //         let file_path = entry.path();
    //         if file_path.is_file() {
    //             let link_path = dirs::home_dir().unwrap().join(".local/bin")
    //                 .join(file_path.file_name().unwrap());
    //             if link_path.exists() {
    //                 fs::remove_file(&link_path)?;
    //             }
    //             unix_fs::symlink(&file_path, &link_path)?;
    //             installed_files.push(link_path);
    //         }
    //     }
    // }

    // let share_dir = package_root.join("share/applications");
    // if share_dir.exists() {
    //     for entry in fs::read_dir(&share_dir)? {
    //         let entry = entry?;
    //         let file_path = entry.path();
    //         if file_path.is_file() {
    //             let link_path = dirs::home_dir().unwrap().join(".local/share/applications")
    //                 .join(file_path.file_name().unwrap());
    //             if link_path.exists() {
    //                 fs::remove_file(&link_path)?;
    //             }
    //             unix_fs::symlink(&file_path, &link_path)?;
    //             installed_files.push(link_path);
    //         }
    //     }
    // }
    if let Ok(symlinks) = symlist::load_symlist(&package_root.join("symlist.ron"), &package_root) {
        for (src_rel, dst_abs) in symlinks {
            let src_abs = package_root.join(src_rel);

            // гарантируем, что директория назначения есть
            if let Some(parent) = dst_abs.parent() {
                std::fs::create_dir_all(parent)?;
            }

            if dst_abs.exists() {
                std::fs::remove_file(&dst_abs)?;
            }

            std::os::unix::fs::symlink(&src_abs, &dst_abs)?;
            info!(
                "Симлинк создан: {} -> {}",
                dst_abs.display(),
                src_abs.display()
            );
            installed_files.push(dst_abs);
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();

    info!("Package {} installed successfully", package_meta.name());
    Ok(())
}

fn unpack(pkg_path: &Path) -> Result<PathBuf, std::io::Error> {
    if pkg_path.extension().and_then(|s| s.to_str()) != Some("uhp") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Package must have .uhp extension",
        ));
    }

    let tmp_dir = dirs::home_dir().unwrap().join(".uhpm/tmp");
    fs::create_dir_all(&tmp_dir)?;

    let package_name = pkg_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown_package");
    let unpack_dir = tmp_dir.join(package_name);

    if unpack_dir.exists() {
        fs::remove_dir_all(&unpack_dir)?;
    }
    fs::create_dir_all(&unpack_dir)?;

    info!("Unpacking {} into {:?}", pkg_path.display(), unpack_dir);

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    info!("Unpacked to {:?}", unpack_dir);
    Ok(unpack_dir)
}
