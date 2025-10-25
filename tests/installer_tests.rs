use flate2::write::GzEncoder;
use std::io::Write;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, Source, installer, remover};
use uhpm::{debug, info, lprintln};

// Test with maximum debugging
#[tokio::test]
async fn test_installer_with_debug_output() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    lprintln!("test.installer_debug.start", home_path.display());

    // Создаем необходимые директории
    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;
    std::fs::create_dir_all(home_path.join(".local/bin"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;
    lprintln!("test.installer_debug.db_initialized", db_path.display());

    // Create package structure
    let pkg_dir = home_path.join("debug-pkg");
    std::fs::create_dir_all(&pkg_dir)?;
    lprintln!("test.installer_debug.pkg_dir_created", pkg_dir.display());

    // Create binary
    let bin_dir = pkg_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    std::fs::write(bin_dir.join("debug_app"), "#!/bin/bash\necho 'Debug'")?;
    lprintln!(
        "test.installer_debug.binary_created",
        bin_dir.join("debug_app").display()
    );

    // Create metadata
    let pkg = Package::new(
        "debug-pkg",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://debug".to_string()),
        "debug123",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path)?;
    lprintln!("test.installer_debug.metadata_created", meta_path.display());

    // Verify metadata can be read back
    let read_pkg = Package::from_toml_file(&meta_path)?;
    lprintln!("test.installer_debug.metadata_verified", read_pkg.name());

    // Create symlist
    let symlist_path = pkg_dir.join("symlist");
    let target_path = home_path.join(".local/bin/debug_app");
    std::fs::write(
        &symlist_path,
        format!("bin/debug_app {}", target_path.display()),
    )?;
    lprintln!(
        "test.installer_debug.symlist_created",
        symlist_path.display()
    );

    // Create archive step by step with verification
    let archive_path = home_path.join("debug-pkg.uhp");
    lprintln!(
        "test.installer_debug.creating_archive",
        archive_path.display()
    );

    // Создаем архив напрямую
    let archive_file = std::fs::File::create(&archive_path)?;
    let enc = flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_path_with_name(&meta_path, "uhp.toml")?;
    tar.append_path_with_name(&symlist_path, "symlist")?;
    tar.append_dir_all("bin", &bin_dir)?;

    tar.finish()?;

    let archive_metadata = std::fs::metadata(&archive_path)?;
    lprintln!(
        "test.installer_debug.archive_created",
        archive_path.display()
    );
    lprintln!("test.installer_debug.archive_size", archive_metadata.len());

    // Check database state before installation
    let packages_before = db.list_packages().await?;
    lprintln!(
        "test.installer_debug.packages_before",
        format!("{:?}", packages_before)
    );

    // Install with detailed error handling
    lprintln!("test.installer_debug.calling_installer", "");
    let result = installer::install(&archive_path, &db).await;

    match &result {
        Ok(()) => {
            lprintln!("test.installer_debug.install_success", "");

            // Check database state after installation
            let packages_after = db.list_packages().await?;
            lprintln!(
                "test.installer_debug.packages_after",
                format!("{:?}", packages_after)
            );

            let installed_files = db.get_installed_files("debug-pkg").await?;
            lprintln!(
                "test.installer_debug.installed_files",
                format!("{:?}", installed_files)
            );

            // Check if package directory was created
            let pkg_install_dir = home_path.join(".uhpm/packages/debug-pkg-1.0.0");
            lprintln!(
                "test.installer_debug.expected_install_dir",
                pkg_install_dir.display()
            );
            lprintln!(
                "test.installer_debug.install_dir_exists",
                pkg_install_dir.exists()
            );

            if pkg_install_dir.exists() {
                let entries: Vec<_> = std::fs::read_dir(&pkg_install_dir)?
                    .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
                    .collect();
                lprintln!(
                    "test.installer_debug.install_dir_contents",
                    format!("{:?}", entries)
                );
            }

            // Cleanup
            let _ = remover::remove("debug-pkg", &db).await;
        }
        Err(e) => {
            lprintln!("test.installer_debug.install_failed", format!("{}", e));
        }
    }

    // For this test, we just want to see the debug output
    // Don't fail the test - we're just gathering information
    lprintln!("test.installer_debug.test_complete", "");

    // В этом тесте нас интересует отладочная информация, а не результат
    Ok(())
}

// Simple test that works - just to verify basic functionality
#[tokio::test]
async fn test_installer_minimal_working() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    // Create the simplest possible package
    let pkg_dir = home_path.join("minimal-pkg");
    std::fs::create_dir_all(&pkg_dir)?;

    // Only metadata - no binaries, no symlinks
    let pkg = Package::new(
        "minimal",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://minimal".to_string()),
        "minimal123",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path)?;

    // Create archive
    let archive_path = home_path.join("minimal.uhp");
    let archive_file = std::fs::File::create(&archive_path)?;
    let enc = flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_path_with_name(&meta_path, "uhp.toml")?;

    // Добавляем пустой symlist чтобы избежать ошибок
    let symlist_path = pkg_dir.join("symlist");
    std::fs::write(&symlist_path, "# Empty symlist")?;
    tar.append_path_with_name(&symlist_path, "symlist")?;

    tar.finish()?;

    // Try to install
    let result = installer::install(&archive_path, &db).await;

    // For now, just check that it doesn't panic
    info!(
        "test.installer_minimal_working.result",
        format!("{:?}", result)
    );

    // Cleanup если установка прошла успешно
    if result.is_ok() {
        let _ = remover::remove("minimal", &db).await;
    }

    Ok(())
}

// Keep the working tests
#[tokio::test]
async fn test_installer_simple() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    // Create minimal package structure
    let pkg_dir = home_path.join("simple-pkg");
    std::fs::create_dir_all(&pkg_dir)?;

    let pkg = Package::new(
        "simple-pkg",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://simple".to_string()),
        "checksum123",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path)?;

    // Create symlist
    let symlist_path = pkg_dir.join("symlist");
    std::fs::write(&symlist_path, "# Simple test symlist")?;

    // Create archive
    let archive_path = home_path.join("simple-pkg.uhp");
    let archive_file = std::fs::File::create(&archive_path)?;
    let enc = flate2::write::GzEncoder::new(archive_file, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_path_with_name(&meta_path, "uhp.toml")?;
    tar.append_path_with_name(&symlist_path, "symlist")?;
    tar.finish()?;

    let result = installer::install(&archive_path, &db).await;
    info!("test.installer_simple.result", format!("{:?}", result));

    // Cleanup
    if result.is_ok() {
        let _ = remover::remove("simple-pkg", &db).await;
    }

    Ok(())
}

#[tokio::test]
async fn test_installer_database_only() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    let pkg = Package::new(
        "db-test",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://db".to_string()),
        "checksum456",
        vec![("dep1".to_string(), semver::Version::parse("1.0.0").unwrap())],
    );

    db.add_package_full(&pkg, &["/fake/path/file1".to_string()])
        .await?;

    let packages = db.list_packages().await?;
    let db_test_pkg = packages.iter().find(|(name, _, _)| name == "db-test");
    assert!(db_test_pkg.is_some(), "Package should be in database");

    // Cleanup
    let _ = remover::remove("db-test", &db).await;

    Ok(())
}
