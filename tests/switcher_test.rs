// tests/switcher_test.rs
//! Integration tests for package version switching functionality

use flate2::Compression;
use flate2::write::GzEncoder;
use semver::Version;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Builder;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, Source, installer, switcher};
use uhpm::{debug, info};

/// Рекурсивное добавление файлов в tar.gz архив с относительными путями
fn append_dir_all(tar: &mut Builder<GzEncoder<File>>, path: &PathBuf, base: &PathBuf) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap();
        if path.is_dir() {
            append_dir_all(tar, &path, base);
        } else {
            tar.append_path_with_name(&path, rel_path).unwrap();
            debug!("test.installer.file_added_to_archive", rel_path);
        }
    }
}

/// Helper для создания архива пакета
fn create_package_archive(tmp_dir: &PathBuf, version: &str) -> std::path::PathBuf {
    let pkg_dir = tmp_dir.join(format!("pkg_{}", version));
    fs::create_dir_all(&pkg_dir.join("bin")).unwrap();

    let bin_file = pkg_dir.join("bin/test_binary");
    fs::write(&bin_file, format!("#!/bin/bash\necho version {}", version)).unwrap();

    let pkg = Package::new(
        "test-package",
        Version::parse(version).unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        format!("checksum-{}", version),
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();

    // Подставляем полный путь к бинарнику в tmp_dir вместо $HOME
    let symlink_target = tmp_dir.join(".local/bin/test_binary");
    fs::write(
        &pkg_dir.join("symlist.ron"),
        format!(
            r#"[(source: "bin/test_binary", target: "{}")]"#,
            symlink_target.display()
        ),
    )
    .unwrap();

    // Создаём архив
    let archive_path = tmp_dir.join(format!("test-package-{}.uhp", version));
    let tar_gz = File::create(&archive_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    append_dir_all(&mut tar, &pkg_dir, &pkg_dir);
    tar.finish().unwrap();
    tar.into_inner().unwrap().finish().unwrap();

    archive_path
}

#[tokio::test]
async fn test_switch_version_success() {
    let _ = tracing_subscriber::fmt::try_init();

    let tmp_dir: PathBuf = tempdir().unwrap().path().to_path_buf();
    let uhpm_root = tmp_dir.join(".uhpm");
    fs::create_dir_all(uhpm_root.join("tmp")).unwrap();
    fs::create_dir_all(tmp_dir.join(".local/bin")).unwrap();

    let db_path = tmp_dir.join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    // Устанавливаем версии 1.0.0 и 2.0.0 через install_at
    let archive_v1 = create_package_archive(&tmp_dir, "1.0.0");
    installer::install_at(&archive_v1, &db, &uhpm_root)
        .await
        .unwrap();

    let archive_v2 = create_package_archive(&tmp_dir, "2.0.0");
    installer::install_at(&archive_v2, &db, &uhpm_root)
        .await
        .unwrap();

    db.set_current_version("test-package", "1.0.0")
        .await
        .unwrap();
    let current_version = db.get_package_version("test-package").await.unwrap();
    assert_eq!(current_version.unwrap(), "1.0.0");

    // Переключаем на 2.0.0
    let result =
        switcher::switch_version("test-package", Version::parse("2.0.0").unwrap(), &db).await;
    assert!(result.is_ok());

    let new_version = db.get_package_version("test-package").await.unwrap();
    assert_eq!(new_version.unwrap(), "2.0.0");
}

#[tokio::test]
async fn test_switch_version_nonexistent() {
    let _ = tracing_subscriber::fmt::try_init();

    let tmp_dir = tempdir().unwrap().path().to_path_buf();
    let uhpm_root = tmp_dir.join(".uhpm");
    fs::create_dir_all(uhpm_root.join("tmp")).unwrap();

    let db_path = tmp_dir.join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    let result =
        switcher::switch_version("nonexistent-package", Version::parse("1.0.0").unwrap(), &db)
            .await;
    assert!(result.is_err());
}
