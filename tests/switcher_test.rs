use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Builder;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, installer};
use uhpm::{debug, info};

/// Рекурсивно добавляет папку и файлы в tar-архив.
fn append_dir_all(tar: &mut Builder<GzEncoder<File>>, path: &Path, base: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap();
        if path.is_dir() {
            append_dir_all(tar, &path, base);
        } else {
            tar.append_path_with_name(&path, rel_path).unwrap();
            debug!("File added to archive: {:?}", rel_path);
        }
    }
}

#[tokio::test]
async fn test_install_simple_package() {
    let _ = tracing_subscriber::fmt().try_init();

    let tmp_dir = tempdir().unwrap();
    info!("Temporary directory: {:?}", tmp_dir.path());

    // Подменяем HOME
    unsafe { std::env::set_var("HOME", tmp_dir.path()) };

    // Создаём структуру пакета
    let pkg_dir = tmp_dir.path().join("pkg_contents");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_file = bin_dir.join("my_binary");
    fs::write(&bin_file, "#!/bin/bash\necho hello").unwrap();
    info!("Binary created: {:?}", bin_file);

    // Генерим uhp.ron
    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();
    info!("Ron's file generated: {:?}", meta_path);

    // Генерим symlist.ron
    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[
    (source: "bin/my_binary", target: "$HOME/.local/bin/my_binary")
]"#,
    )
    .unwrap();
    info!("Symlist generated: {:?}", symlist_path);

    // Создаём папку для установки бинарников
    fs::create_dir_all(tmp_dir.path().join(".local/bin")).unwrap();

    // Создаём архив .uhp
    let uhp_path = tmp_dir.path().join("my_package.uhp");
    let tar_gz = File::create(&uhp_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    append_dir_all(&mut tar, &pkg_dir, &pkg_dir);
    tar.finish().unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    info!("Package archive created: {:?}", &uhp_path);

    // Инициализируем базу данных
    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!("Database initialized: {:?}", db_path);

    // **Устанавливаем пакет через installer**
    installer::install(&uhp_path, &db).await.unwrap();
    info!("Package installed successfully");

    // Проверяем версию пакета
    let version = db.get_package_version("my_package").await.unwrap();
    info!("Installed package version: {:?}", &version);
    assert!(version.is_some(), "Package not added to database");
    assert_eq!(version.unwrap(), "0.1.0");

    // Проверяем установленные файлы
    let installed_files = db.get_installed_files("my_package").await.unwrap();
    info!("Installed files: {:?}", &installed_files);
    assert!(!installed_files.is_empty(), "Files not installed");
}
