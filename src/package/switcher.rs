use crate::db::PackageDB;
use crate::package::installer::create_symlinks;
use semver::Version;
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Debug)]
pub enum SwitchError {
    Io(std::io::Error),
    Db(sqlx::Error),
    MissingPackageDir(PathBuf),
    Symlist(crate::symlist::SymlistError),
    PackageNotFound(String, Version),
}

impl From<std::io::Error> for SwitchError {
    fn from(e: std::io::Error) -> Self {
        SwitchError::Io(e)
    }
}
impl From<sqlx::Error> for SwitchError {
    fn from(e: sqlx::Error) -> Self {
        SwitchError::Db(e)
    }
}
impl From<crate::symlist::SymlistError> for SwitchError {
    fn from(e: crate::symlist::SymlistError) -> Self {
        SwitchError::Symlist(e)
    }
}

pub async fn switch_version(
    pkg_name: &str,
    target_version: Version,
    db: &PackageDB,
) -> Result<(), SwitchError> {

    // let pkg: Package = db
    //     .get_package_by_version(pkg_name, &target_version.to_string())
    //     .await?
    //     .ok_or(SwitchError::PackageNotFound(pkg_name.to_string(), target_version.clone()))?;


    if let Some(current_package) = db.get_current_package(pkg_name).await? {
        let current_version_str = current_package.version().to_string();
        let current_pkg_dir = dirs::home_dir()
            .unwrap()
            .join(".uhpm/packages")
            .join(format!("{}-{}", pkg_name, current_version_str));

        if current_pkg_dir.exists() {
            let symlist_path = current_pkg_dir.join("symlist.ron");
            match crate::symlist::load_symlist(&symlist_path, &current_pkg_dir) {
                Ok(symlinks) => {
                    for (src_abs, dst_abs) in symlinks {
                        if !dst_abs.exists() {
                            continue;
                        }

                        match std::fs::symlink_metadata(&dst_abs) {
                            Ok(meta) if meta.file_type().is_symlink() => match std::fs::read_link(&dst_abs) {
                                Ok(link_target) if link_target == src_abs => {
                                    if let Err(e) = std::fs::remove_file(&dst_abs) {
                                        warn!("Не удалось удалить симлинк {}: {}", dst_abs.display(), e);
                                    } else {
                                        info!("Удалён старый симлинк: {}", dst_abs.display());
                                    }
                                }
                                Ok(link_target) => {
                                    info!(
                                        "Пропускаю {} — симлинк указывает не на пакет (ожидалось: {}, реальность: {})",
                                        dst_abs.display(),
                                        src_abs.display(),
                                        link_target.display()
                                    );
                                }
                                Err(e) => {
                                    warn!("Не удалось прочитать цель симлинка {}: {}", dst_abs.display(), e);
                                }
                            },
                            Ok(_) => info!("Пропускаю {} — не симлинк.", dst_abs.display()),
                            Err(e) => warn!("Не удалось получить метаданные {}: {}", dst_abs.display(), e),
                        }
                    }
                }
                Err(crate::symlist::SymlistError::Io(ref io_err))
                    if io_err.kind() == std::io::ErrorKind::NotFound =>
                {
                    info!("symlist.ron для текущей версии не найден, пропуск удаления симлинков");
                }
                Err(e) => return Err(SwitchError::Symlist(e)),
            }
        } else {
            info!(
                "Папка текущей версии пакета не найдена ({}), пропускаем удаление симлинков",
                current_pkg_dir.display()
            );
        }
    } else {
        info!("Текущая версия пакета не записана в базе — пропускаем удаление симлинков");
    }


    let new_pkg_dir = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, target_version));

    if !new_pkg_dir.exists() {
        return Err(SwitchError::MissingPackageDir(new_pkg_dir));
    }

    create_symlinks(&new_pkg_dir)?;

    db.set_current_version(pkg_name, &target_version.to_string())
        .await?;

    info!(
        "Пакет '{}' переключён на версию {} (симлинки обновлены).",
        pkg_name, target_version
    );

    Ok(())
}
