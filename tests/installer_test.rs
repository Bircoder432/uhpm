use uhpm::package::{installer, Package};
use uhpm::db::PackageDB;
use std::fs::{self, File};
use tempfile::tempdir;
use tar::Builder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::path::{Path, PathBuf};
use tracing::{info, debug};

fn append_dir_all(tar: &mut Builder<GzEncoder<File>>, path: &Path, base: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap();
        if path.is_dir() {
            append_dir_all(tar, &path, base);
        } else {
            tar.append_path_with_name(&path, rel_path).unwrap();
            debug!("Добавлен файл в архив: {:?}", rel_path);
        }
    }
}

#[tokio::test]
async fn test_install_simple_package() {
    // Инициализация логов
    let _ = tracing_subscriber::fmt().try_init();

    // Временная директория
    let tmp_dir = tempdir().unwrap();
    info!("TMP_DIR = {:?}", tmp_dir.path());

    // Переназначаем HOME
    unsafe {
    std::env::set_var("HOME", tmp_dir.path());
    }
    info!("HOME переназначен на {:?}", tmp_dir.path());

    // Создаём содержимое пакета
    let pkg_dir = tmp_dir.path().join("pkg_contents");
    fs::create_dir_all(&pkg_dir).unwrap();

    // bin/my_binary
    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_file = bin_dir.join("my_binary");
    fs::write(&bin_file, "#!/bin/bash\necho hello").unwrap();
    info!("Создан бинарник: {:?}", bin_file);

    // uhp.ron
    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();
    let ron_content = fs::read_to_string(&meta_path).unwrap();
    info!("Сгенерирован uhp.ron:\n{}", ron_content);

    // symlist.ron
    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[
    (source: "bin/my_binary", target: "$HOME/.local/bin/my_binary")
]"#,
    )
    .unwrap();
    let symlist_content = fs::read_to_string(&symlist_path).unwrap();
    info!("Сгенерирован symlist.ron:\n{}", symlist_content);

    // Директория для симлинков
    fs::create_dir_all(tmp_dir.path().join(".local/bin")).unwrap();

    // Создаём архив .uhp рекурсивно
    let uhp_path = tmp_dir.path().join("my_package.uhp");
    info!("Архив будет создан в {:?}", uhp_path);
    let tar_gz = File::create(&uhp_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    append_dir_all(&mut tar, &pkg_dir, &pkg_dir);
    tar.finish().unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    info!("Архив создан: {:?}", uhp_path);

    // Создаём базу
    let db_path = tmp_dir.path().join("packages.db");
    info!("База данных будет создана в {:?}", db_path);
    let db = PackageDB::new(&db_path).await.unwrap();

    // Установка пакета
    info!("Начинаем установку пакета");
    installer::install(&uhp_path, &db).await.unwrap();
    info!("Установка пакета завершена");

    // Устанавливаем current версию
    db.set_current_version("my_package", &pkg.version().to_string())
        .await
        .unwrap();

    // Проверка версии пакета
    let version = db.get_package_version("my_package").await.unwrap();
    info!("Версия пакета после установки: {:?}", version);
    assert!(version.is_some(), "Пакет не добавлен в базу!");
    assert_eq!(version.unwrap(), "0.1.0");

    // Проверка установленных файлов
    let installed_files = db.get_installed_files("my_package").await.unwrap();
    info!("Установленные файлы: {:?}", installed_files);
    assert!(!installed_files.is_empty(), "Файлы пакета не добавлены!");
}
