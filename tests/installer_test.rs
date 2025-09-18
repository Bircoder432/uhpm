use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uhpm::package::{Package, Source};

// Мокаем PackageDB
#[async_trait]
pub trait PackageDBTrait {
    async fn get_package_version(&self, name: &str) -> Option<String>;
    async fn get_installed_files(&self, name: &str) -> Vec<String>;
}

struct MockPackageDB;

#[async_trait]
impl PackageDBTrait for MockPackageDB {
    async fn get_package_version(&self, _name: &str) -> Option<String> {
        Some("0.1.0".to_string())
    }

    async fn get_installed_files(&self, _name: &str) -> Vec<String> {
        vec!["bin/my_binary".to_string()]
    }
}

// Мокаем installer::install
async fn install_mock(
    _archive: &Path,
    _db: &impl PackageDBTrait,
    _symlinks: &HashMap<String, String>,
) -> Result<(), String> {
    Ok(())
}

#[tokio::test]
async fn test_install_simple_package_mocked() {
    // Подготовка мок-базы
    let db = MockPackageDB;

    // Тестовый архив (можно просто PathBuf::new() если не читаем его)
    let fake_archive = PathBuf::from("/fake/path/my_package.uhp");

    // Тестируем вызов "установки"
    let symlinks = HashMap::new();
    install_mock(&fake_archive, &db, &symlinks).await.unwrap();

    // Проверка "базы"
    let version = db.get_package_version("my_package").await.unwrap();
    assert_eq!(version, "0.1.0");

    let files = db.get_installed_files("my_package").await;
    assert_eq!(files, vec!["bin/my_binary"]);
}
