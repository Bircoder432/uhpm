use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{installer, remover};

#[tokio::test]
async fn test_install_nonexistent_archive() {
    let tmp_dir = tempdir().unwrap();
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    // Создаем необходимые директории
    std::fs::create_dir_all(home_path.join(".uhpm/packages")).unwrap();

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    let result = installer::install(&home_path.join("nonexistent.uhp"), &db).await;
    assert!(result.is_err(), "Should fail on nonexistent archive");
}

#[tokio::test]
async fn test_install_corrupted_archive() {
    let tmp_dir = tempdir().unwrap();
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages")).unwrap();

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    // Create a corrupted archive
    let corrupted_path = home_path.join("corrupted.uhp");
    std::fs::write(&corrupted_path, "not a valid tar.gz file").unwrap();

    let result = installer::install(&corrupted_path, &db).await;
    assert!(result.is_err(), "Should fail on corrupted archive");
}

#[tokio::test]
async fn test_remove_nonexistent_package() {
    let tmp_dir = tempdir().unwrap();
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages")).unwrap();

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    let result = remover::remove("nonexistent-package", &db).await;
    assert!(
        result.is_ok(),
        "Removing nonexistent package should not fail"
    );
}

#[tokio::test]
async fn test_install_missing_metadata() {
    let tmp_dir = tempdir().unwrap();
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages")).unwrap();

    // Create archive without uhp.toml
    let pkg_dir = home_path.join("invalid-pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    // Создаем файл, но не uhp.toml
    std::fs::write(pkg_dir.join("some_file.txt"), "just a file").unwrap();

    let archive_path = home_path.join("invalid.uhp");
    let tar_gz = std::fs::File::create(&archive_path).unwrap();
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(".", &pkg_dir).unwrap();
    tar.finish().unwrap();

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    let result = installer::install(&archive_path, &db).await;
    assert!(result.is_err(), "Should fail on missing metadata");
}
