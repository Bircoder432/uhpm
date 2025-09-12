// tests/switcher_test.rs
//! Integration tests for package version switching functionality

use semver::Version;
use std::fs;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, Source, installer, switcher};
use uhpm::{debug, error, info};

#[tokio::test]
async fn test_switch_version_success() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!("test.switcher_test.setup_tmp_dir", tmp_dir.path());

    // Create target directory first
    let target_dir = tmp_dir.path().join(".local/bin");
    fs::create_dir_all(&target_dir).unwrap();
    debug!(
        "test.switcher_test.created_target_dir",
        target_dir.display()
    );

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!("test.switcher_test.db_initialized", db_path.display());

    // Install version 1.0.0
    let pkg_tmp_dir = tempdir().unwrap();
    let pkg_dir = pkg_tmp_dir.path().join("test-package");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(
        bin_dir.join("test_binary"),
        "#!/bin/bash\necho version 1.0.0",
    )
    .unwrap();

    let pkg = Package::new(
        "test-package",
        Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "checksum-1.0.0",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();

    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[(source: "bin/test_binary", target: "$HOME/.local/bin/test_binary")]"#,
    )
    .unwrap();

    let archive_path = tmp_dir.path().join("test-package-1.0.0.uhp");
    {
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", &pkg_dir).unwrap();
        tar.finish().unwrap();
    }

    installer::install(&archive_path, &db).await.unwrap();
    info!("test.switcher_test.package_1_installed");

    // Install version 2.0.0
    let pkg_tmp_dir2 = tempdir().unwrap();
    let pkg_dir2 = pkg_tmp_dir2.path().join("test-package");
    fs::create_dir_all(&pkg_dir2).unwrap();

    let bin_dir2 = pkg_dir2.join("bin");
    fs::create_dir_all(&bin_dir2).unwrap();
    fs::write(
        bin_dir2.join("test_binary"),
        "#!/bin/bash\necho version 2.0.0",
    )
    .unwrap();

    let pkg2 = Package::new(
        "test-package",
        Version::parse("2.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "checksum-2.0.0",
        vec![],
    );

    let meta_path2 = pkg_dir2.join("uhp.ron");
    pkg2.save_to_ron(&meta_path2).unwrap();

    let symlist_path2 = pkg_dir2.join("symlist.ron");
    fs::write(
        &symlist_path2,
        r#"[(source: "bin/test_binary", target: "$HOME/.local/bin/test_binary")]"#,
    )
    .unwrap();

    let archive_path2 = tmp_dir.path().join("test-package-2.0.0.uhp");
    {
        let tar_gz = fs::File::create(&archive_path2).unwrap();
        let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", &pkg_dir2).unwrap();
        tar.finish().unwrap();
    }

    installer::install(&archive_path2, &db).await.unwrap();
    info!("test.switcher_test.package_2_installed");

    // Set initial version to 1.0.0
    db.set_current_version("test-package", "1.0.0")
        .await
        .unwrap();
    info!("test.switcher_test.initial_version_set");

    // Verify initial version
    let current_version = db.get_package_version("test-package").await.unwrap();
    assert_eq!(current_version.unwrap(), "1.0.0");
    info!("test.switcher_test.initial_version_verified");

    // Check if symlink exists before switching
    let symlink_path = tmp_dir.path().join(".local/bin/test_binary");
    if symlink_path.exists() {
        debug!("test.switcher_test.symlink_exists", symlink_path.display());
        if symlink_path.is_symlink() {
            match std::fs::read_link(&symlink_path) {
                Ok(target) => debug!("test.switcher_test.symlink_target", target.display()),
                Err(e) => error!("test.switcher_test.symlink_read_error", e),
            }
        }
    } else {
        debug!("test.switcher_test.symlink_not_found");
    }

    // Switch to version 2.0.0
    let result =
        switcher::switch_version("test-package", Version::parse("2.0.0").unwrap(), &db).await;
    assert!(result.is_ok(), "Switch failed: {:?}", result.err());
    info!("test.switcher_test.switch_successful");

    // Verify version switched in database
    let new_version = db.get_package_version("test-package").await.unwrap();
    assert_eq!(new_version.unwrap(), "2.0.0");
    info!("test.switcher_test.version_switched_verified");

    // Check symlink after switching
    if symlink_path.exists() {
        debug!(
            "test.switcher_test.symlink_exists_after",
            symlink_path.display()
        );
        if symlink_path.is_symlink() {
            match std::fs::read_link(&symlink_path) {
                Ok(target) => debug!("test.switcher_test.symlink_target_after", target.display()),
                Err(e) => error!("test.switcher_test.symlink_read_error_after", e),
            }
        }
    }
}

#[tokio::test]
async fn test_switch_version_nonexistent() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!(
        "test.switcher_test.setup_tmp_dir_nonexistent",
        tmp_dir.path()
    );

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!(
        "test.switcher_test.db_initialized_nonexistent",
        db_path.display()
    );

    // Try to switch non-existent package
    let result =
        switcher::switch_version("nonexistent-package", Version::parse("1.0.0").unwrap(), &db)
            .await;
    assert!(result.is_err(), "Should fail for non-existent package");
    info!("test.switcher_test.nonexistent_switch_failed");
}

#[tokio::test]
async fn test_switch_version_missing_target() {
    let _ = tracing_subscriber::fmt::try_init();
    let tmp_dir = tempdir().unwrap();
    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!("test.switcher_test.setup_tmp_dir_missing", tmp_dir.path());

    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!(
        "test.switcher_test.db_initialized_missing",
        db_path.display()
    );

    // Create and install only version 1.0.0
    let pkg_tmp_dir = tempdir().unwrap();
    let pkg_dir = pkg_tmp_dir.path().join("test-package");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fs::write(
        bin_dir.join("test_binary"),
        "#!/bin/bash\necho version 1.0.0",
    )
    .unwrap();

    let pkg = Package::new(
        "test-package",
        Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "checksum-1.0.0",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();

    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[(source: "bin/test_binary", target: "$HOME/.local/bin/test_binary")]"#,
    )
    .unwrap();

    let archive_path = tmp_dir.path().join("test-package-1.0.0.uhp");
    {
        let tar_gz = fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", &pkg_dir).unwrap();
        tar.finish().unwrap();
    }

    installer::install(&archive_path, &db).await.unwrap();
    info!("test.switcher_test.package_installed_missing");

    // Try to switch to non-existent version 2.0.0
    let result =
        switcher::switch_version("test-package", Version::parse("2.0.0").unwrap(), &db).await;
    assert!(result.is_err(), "Should fail for missing target version");
    info!("test.switcher_test.missing_target_switch_failed");
}
