use crate::db::PackageDB;
use crate::package::{Package, Source};
use crate::symlist;
use semver::Version;
use tracing::instrument::WithSubscriber;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, debug, warn, error};

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
    info!("Начало установки пакета: {}", pkg_path.display());


    let unpacked = unpack(pkg_path)?;
    debug!("Архив распакован в {:?}", unpacked);


    let meta_path = unpacked.join("uhp.ron");
    debug!("Чтение метаданных из {:?}", meta_path);
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!("Пакет: {} версия {}", package_meta.name(), package_meta.version());

    let pkg_name = package_meta.name();
    let version = package_meta.version();


    let already_installed = db.is_installed(pkg_name).await.unwrap();
    if let Some(installed_version) = &already_installed {
        info!(
            "Пакет {} уже установлен с версией {}",
            pkg_name, installed_version
        );
        if installed_version == version {
            info!("Версия совпадает — установка отменена");
            return Ok(());
        }
    }


    let package_root = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, version));
    debug!("Путь установки пакета: {:?}", package_root);

    if package_root.exists() {
        debug!("Путь уже существует, удаляем: {:?}", package_root);
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;
    debug!("Папка пакета создана");


    fs::rename(&unpacked, &package_root)?;
    debug!("Пакет перемещён в {:?}", package_root);


    let mut installed_files = Vec::new();
    match already_installed {
        None => {
            info!("Создание симлинков для нового пакета");
            installed_files = create_symlinks(&package_root)?;
        }
        Some(_) => {
            info!("Обновление версии пакета — симлинки не создаются");
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    info!(
        "Добавляем пакет {} в базу с {} файлами",
        pkg_name,
        installed_files_str.len()
    );
    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();
    db.set_current_version(&package_meta.name(), &package_meta.version().to_string()).await.unwrap();

    info!("Установка пакета {} завершена успешно", pkg_name);
    Ok(())
}


pub fn create_symlinks(package_root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut installed_files = Vec::new();

    let symlist_path = package_root.join("symlist.ron");
    debug!("Загружаем symlist из {:?}", symlist_path);

    match symlist::load_symlist(&symlist_path, &package_root) {
        Ok(symlinks) => {
            for (src_rel, dst_abs) in symlinks {
                let src_abs = package_root.join(&src_rel);
                debug!("Обрабатываем симлинк: {} -> {}", src_abs.display(), dst_abs.display());

                if !src_abs.exists() {
                    warn!("Файл источника не найден: {}", src_abs.display());
                    continue;
                }

                if let Some(parent) = dst_abs.parent() {
                    fs::create_dir_all(parent)?;
                    debug!("Создана директория для симлинка: {:?}", parent);
                }

                if dst_abs.exists() {
                    fs::remove_file(&dst_abs)?;
                    debug!("Удалён существующий симлинк: {:?}", dst_abs);
                }

                std::os::unix::fs::symlink(&src_abs, &dst_abs)?;
                debug!("Симлинк создан: {} -> {}", dst_abs.display(), src_abs.display());
                installed_files.push(dst_abs);

            }
        }
        Err(e) => {
            warn!("Не удалось загрузить symlist: {:?}", e);
        }
    }

    debug!("Всего создано {} симлинков", installed_files.len());
    Ok(installed_files)
}

/// Распаковка архива с debug-логами
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

    debug!("Распаковка {} в {:?}", pkg_path.display(), unpack_dir);

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    debug!("Распаковано в {:?}", unpack_dir);
    Ok(unpack_dir)
}
