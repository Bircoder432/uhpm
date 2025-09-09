
use crate::db::PackageDB;
use crate::package::Package;
use crate::package::installer::create_symlinks;
use crate::symlist;
use std::path::PathBuf;
use std::io::ErrorKind;
use semver::Version;
use tracing::{info, instrument::WithSubscriber, warn};

#[derive(Debug)]
pub enum SwitchError {
    Io(std::io::Error),
    Db(sqlx::Error),
    MissingPackageDir(PathBuf),
    Symlist(crate::symlist::SymlistError),
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


pub async fn switch_version(pkg_name: &str,target_version: Version, db: &PackageDB) -> Result<(), SwitchError> {

    let pkg: Package = db.get_package_by_version(pkg_name, &target_version.to_string()).await.unwrap().unwrap();

    let current_package: Package = db.get_current_package(&pkg_name).await.unwrap().unwrap();
    let current_version_opt: &str = &current_package.version().to_string();

    if let current_version_str = current_version_opt {
        let current_pkg_dir = dirs::home_dir()
            .unwrap()
            .join(".uhpm/packages")
            .join(format!("{}-{}", pkg_name, current_version_str));

        if current_pkg_dir.exists() {
            let symlist_path = current_pkg_dir.join("symlist.ron");
            match crate::symlist::load_symlist(&symlist_path, &current_pkg_dir) {
                Ok(symlinks) => {
                    for (src_abs, dst_abs) in symlinks {

                        if dst_abs.exists() {
                            match std::fs::symlink_metadata(&dst_abs) {
                                Ok(meta) => {
                                    if meta.file_type().is_symlink() {
                                        match std::fs::read_link(&dst_abs) {
                                            Ok(link_target) => {

                                                if link_target == src_abs {

                                                    if let Err(e) = std::fs::remove_file(&dst_abs) {
                                                        warn!(
                                                            "Не удалось удалить симлинк {}: {}",
                                                            dst_abs.display(),
                                                            e
                                                        );
                                                    } else {
                                                        info!("Удалён старый симлинк: {}", dst_abs.display());
                                                    }
                                                } else {
                                                    info!(
                                                        "Пропускаю {} — симлинк указывает не на пакет (ожидалось: {}, реальность: {})",
                                                        dst_abs.display(),
                                                        src_abs.display(),
                                                        link_target.display()
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                warn!("Не удалось прочитать цель симлинка {}: {}", dst_abs.display(), e);
                                            }
                                        }
                                    } else {
                                        info!("Пропускаю {} — не симлинк.", dst_abs.display());
                                    }
                                }
                                Err(e) => {
                                    warn!("Не удалось получить метаданные {}: {}", dst_abs.display(), e);
                                }
                            }
                        }
                    }
                }
                Err(crate::symlist::SymlistError::Io(ref io_err)) if io_err.kind() == ErrorKind::NotFound => {

                    info!("symlist.ron для текущей версии не найден, пропуск удаления симлинков");
                }
                Err(e) => {

                    return Err(SwitchError::Symlist(e));
                }
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


    let new_installed_files = create_symlinks(&new_pkg_dir)?;


    db.set_current_version(pkg_name, &target_version.to_string()).await.unwrap();

    info!(
        "Пакет '{}' переключён на версию {} (симлинки обновлены).",
        pkg_name, target_version
    );

    Ok(())
}
