use uhpm::package::{installer, Package, meta_parser};
use uhpm::db::PackageDB;
use semver::Version;
use std::fs::{self, File};
use std::path::PathBuf;
use tempfile::tempdir;
use tar::Builder;
use flate2::write::GzEncoder;
use flate2::Compression;

#[tokio::test]
async fn test_install_simple_package() {
    // Временная директория для пакета
    let tmp_dir = tempdir().unwrap();
    let pkg_dir = tmp_dir.path().join("my_package_dir");
    fs::create_dir_all(&pkg_dir).unwrap();

    // Создаём uhp.ron внутри директории пакета
    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    let ron_str = ron::ser::to_string(&pkg).unwrap();
    fs::write(&meta_path, ron_str).unwrap();

    // Создаём архив .uhp с uhp.ron внутри
    let uhp_path = tmp_dir.path().join("my_package.uhp");
    let tar_gz = File::create(&uhp_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    tar.append_path_with_name(&meta_path, "uhp.ron").unwrap();
    tar.finish().unwrap();

    // Временная SQLite база
    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).await.unwrap();

    // Устанавливаем пакет
    installer::install(&uhp_path, &db).await.unwrap();

    // Проверяем, что пакет появился в базе
    let version = db.get_package_version("my_package").await.unwrap();
    assert_eq!(version.unwrap(), "0.1.0");

    // Проверяем установленные файлы
    let installed_files = db.get_installed_files("my_package").await.unwrap();
    assert!(!installed_files.is_empty());
}
