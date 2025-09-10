use crate::db::PackageDB;

#[derive(Debug)]
pub enum DeleteError {
    Io(std::io::Error),
    Db(sqlx::Error),
}

impl From<std::io::Error> for DeleteError {
    fn from(e: std::io::Error) -> Self {
        DeleteError::Io(e)
    }
}

impl From<sqlx::Error> for DeleteError {
    fn from(e: sqlx::Error) -> Self {
        DeleteError::Db(e)
    }
}

pub async fn remove(pkg_name: &str, db: &PackageDB) -> Result<(), DeleteError> {

    let version = db.get_package_version(pkg_name).await?;
    if version.is_none() {
        tracing::warn!("Пакет '{}' не найден в базе", pkg_name);
        return Ok(());
    }
    let version = version.unwrap();

    tracing::info!("Попытка удалить пакет: {}-{}", pkg_name, version);

    let mut pkg_dir = dirs::home_dir().unwrap();
    pkg_dir.push(".uhpm/packages");
    pkg_dir.push(format!("{}-{}", pkg_name, version));

    if pkg_dir.exists() {
        std::fs::remove_dir_all(&pkg_dir)?;
        tracing::info!("Папка пакета удалена: {}", pkg_dir.display());
    } else {
        tracing::warn!(
            "Папка пакета '{}' не найдена: {}",
            pkg_name,
            pkg_dir.display()
        );
    }

    let files: Vec<String> = db.get_installed_files(pkg_name).await?;
    for f in files {
        let path = std::path::PathBuf::from(f);
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            tracing::info!("Удалён: {}", path.display());
        }
    }

    db.remove_package(pkg_name).await?;
    tracing::info!("Запись пакета '{}' удалена из базы", pkg_name);

    Ok(())
}
