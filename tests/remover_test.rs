// tests/remover_test.rs
//! Integration tests for package removal functionality

use semver::Version;
use std::fs;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::remover;
use uhpm::package::{Package, Source, installer};
use uhpm::{debug, info};

#[tokio::test]
async fn test_remove_package_success() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!("test.remover_test.setup_tmp_dir", tmp_dir.path());

    let pkg_dir = tmp_dir.path().join("test-package");
    fs::create_dir_all(&pkg_dir).unwrap();
    debug!("test.remover_test.created_pkg_dir", pkg_dir.display());

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(bin_dir.join("test_binary"), "#!/bin/bash\necho test").unwrap();
    info!(
        "test.remover_test.created_binary",
        bin_dir.join("test_binary").display()
    );

    let pkg = Package::new(
        "test-package",
        Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "test-checksum",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();
    info!("test.remover_test.created_meta", meta_path.display());

    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[(source: "bin/test_binary", target: "$HOME/.local/bin/test_binary")]"#,
    )
    .unwrap();
    info!("test.remover_test.created_symlist", symlist_path.display());

    fs::create_dir_all(tmp_dir.path().join(".local/bin")).unwrap();
    debug!("test.remover_test.created_target_dir");

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!("test.remover_test.db_initialized", db_path.display());

    let archive_path = tmp_dir.path().join("test-package.uhp");
    {
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", &pkg_dir).unwrap();
        tar.finish().unwrap();
    }
    info!("test.remover_test.archive_created", archive_path.display());

    installer::install(&archive_path, &db).await.unwrap();
    info!("test.remover_test.package_installed");

    let installed_version = db.get_package_version("test-package").await.unwrap();
    assert!(installed_version.is_some());
    info!("test.remover_test.installation_verified");

    let result = remover::remove("test-package", &db).await;
    assert!(result.is_ok(), "Removal failed: {:?}", result.err());
    info!("test.remover_test.removal_successful");

    let removed_version = db.get_package_version("test-package").await.unwrap();
    assert!(removed_version.is_none());
    info!("test.remover_test.db_entry_removed");

    let symlink_path = tmp_dir.path().join(".local/bin/test_binary");
    assert!(!symlink_path.exists());
    info!("test.remover_test.symlink_removed");

    let pkg_install_dir = tmp_dir.path().join(".uhpm/packages/test-package-1.0.0");
    assert!(!pkg_install_dir.exists());
    info!("test.remover_test.pkg_dir_removed");
}

#[tokio::test]
async fn test_remove_nonexistent_package() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!(
        "test.remover_test.setup_tmp_dir_nonexistent",
        tmp_dir.path()
    );

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!(
        "test.remover_test.db_initialized_nonexistent",
        db_path.display()
    );

    let result = remover::remove("nonexistent-package", &db).await;
    assert!(
        result.is_ok(),
        "Removing non-existent package should not fail"
    );
    info!("test.remover_test.nonexistent_removal_ok");
}

#[tokio::test]
async fn test_remove_package_missing_directory() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!("test.remover_test.setup_tmp_dir_missing", tmp_dir.path());

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!(
        "test.remover_test.db_initialized_missing",
        db_path.display()
    );

    let pkg = Package::new(
        "missing-dir-package",
        Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "test-checksum",
        vec![],
    );

    db.add_package_full(&pkg, &["/some/missing/file".to_string()])
        .await
        .unwrap();
    info!("test.remover_test.package_added_to_db");

    let result = remover::remove("missing-dir-package", &db).await;
    assert!(
        result.is_ok(),
        "Removal should succeed despite missing directory"
    );
    info!("test.remover_test.missing_dir_removal_ok");

    let removed_version = db.get_package_version("missing-dir-package").await.unwrap();
    assert!(removed_version.is_none());
    info!("test.remover_test.db_entry_removed_missing");
}
