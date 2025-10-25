use std::path::{Path, PathBuf};
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, Source, installer, remover, switcher};
use uhpm::{debug, info};

// Вспомогательные функции для создания тестовых пакетов
fn create_test_package(pkg_dir: &Path, name: &str, version: &str) -> Package {
    let bin_dir = pkg_dir.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::write(bin_dir.join("test_binary"), "#!/bin/bash\necho test").unwrap();

    let pkg = Package::new(
        name,
        semver::Version::parse(version).unwrap(),
        "Test Author",
        Source::Raw("test://package".to_string()),
        "test-checksum",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path).unwrap();

    // Создаем правильный symlist с абсолютными путями
    let symlist_path = pkg_dir.join("symlist");
    let test_binary_path = bin_dir.join("test_binary");
    std::fs::write(
        &symlist_path,
        format!(
            "bin/test_binary {}/.local/bin/test_binary",
            std::env::var("HOME").unwrap()
        ),
    )
    .unwrap();

    pkg
}

fn create_test_archive(
    pkg_dir: &Path,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let tar_gz = std::fs::File::create(output_path)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Добавляем файлы в архив с правильными путями
    tar.append_path_with_name(pkg_dir.join("uhp.toml"), "uhp.toml")?;
    tar.append_path_with_name(pkg_dir.join("symlist"), "symlist")?;

    // Добавляем bin директорию
    let bin_dir = pkg_dir.join("bin");
    if bin_dir.exists() {
        tar.append_dir_all("bin", &bin_dir)?;
    }

    tar.finish()?;
    Ok(())
}

#[tokio::test]
async fn test_package_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();

    // Устанавливаем HOME переменную безопасно
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    info!("test.integration.lifecycle.start", home_path.display());

    // Setup directories
    std::fs::create_dir_all(home_path.join(".local/bin"))?;
    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    // Create and install package v1.0.0
    let pkg_dir_v1 = home_path.join("pkg-v1");
    std::fs::create_dir_all(&pkg_dir_v1)?;
    create_test_package(&pkg_dir_v1, "test-package", "1.0.0");

    let archive_v1 = home_path.join("test-package-1.0.0.uhp");
    create_test_archive(&pkg_dir_v1, &archive_v1)?;

    installer::install(&archive_v1, &db).await?;
    info!("test.integration.lifecycle.installed_v1");

    // Verify installation
    let version = db.get_package_version("test-package").await?;
    assert_eq!(version, Some("1.0.0".to_string()));

    let symlink_path = home_path.join(".local/bin/test_binary");

    // Проверяем, что пакет установился в правильную директорию
    let pkg_install_dir = home_path.join(".uhpm/packages/test-package-1.0.0");
    debug!(
        "test.integration.lifecycle.pkg_install_dir",
        pkg_install_dir.display()
    );

    assert!(
        pkg_install_dir.exists(),
        "Package install directory should exist"
    );

    // Проверяем, что файлы распаковались
    let installed_meta = pkg_install_dir.join("uhp.toml");
    let installed_symlist = pkg_install_dir.join("symlist");
    let installed_binary = pkg_install_dir.join("bin/test_binary");

    assert!(installed_meta.exists(), "Metadata should be extracted");
    assert!(installed_symlist.exists(), "Symlist should be extracted");
    assert!(installed_binary.exists(), "Binary should be extracted");

    // Create and install package v2.0.0
    let pkg_dir_v2 = home_path.join("pkg-v2");
    std::fs::create_dir_all(&pkg_dir_v2)?;
    create_test_package(&pkg_dir_v2, "test-package", "2.0.0");

    let archive_v2 = home_path.join("test-package-2.0.0.uhp");
    create_test_archive(&pkg_dir_v2, &archive_v2)?;

    installer::install(&archive_v2, &db).await?;
    info!("test.integration.lifecycle.installed_v2");

    // Verify both versions are in database
    let packages = db.list_packages().await?;
    let test_packages: Vec<_> = packages
        .iter()
        .filter(|(name, _, _)| name == "test-package")
        .collect();
    assert_eq!(
        test_packages.len(),
        2,
        "Should have both versions in database"
    );

    // Switch between versions
    switcher::switch_version(
        "test-package",
        semver::Version::parse("1.0.0").unwrap(),
        &db,
    )
    .await?;
    info!("test.integration.lifecycle.switched_to_v1");

    // Verify switch worked
    let current_version = db.get_package_version("test-package").await?;
    assert_eq!(current_version, Some("1.0.0".to_string()));

    switcher::switch_version(
        "test-package",
        semver::Version::parse("2.0.0").unwrap(),
        &db,
    )
    .await?;
    info!("test.integration.lifecycle.switched_to_v2");

    // Verify switch worked
    let current_version = db.get_package_version("test-package").await?;
    assert_eq!(current_version, Some("2.0.0".to_string()));

    // Remove package
    remover::remove("test-package", &db).await?;
    info!("test.integration.lifecycle.removed");

    // Verify removal
    let version_after_removal = db.get_package_version("test-package").await?;
    assert!(
        version_after_removal.is_none(),
        "Package should be removed from database"
    );

    // Проверяем, что файлы пакета удалены
    assert!(
        !pkg_install_dir.exists(),
        "Package directory should be removed"
    );

    Ok(())
}

// Упрощенный тест для проверки базовой функциональности
#[tokio::test]
async fn test_basic_install_remove() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    // Создаем необходимые директории
    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    // Create package
    let pkg_dir = home_path.join("test-pkg");
    std::fs::create_dir_all(&pkg_dir)?;

    let bin_dir = pkg_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    std::fs::write(bin_dir.join("app"), "#!/bin/bash\necho hello")?;

    let pkg = Package::new(
        "test-app",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://app".to_string()),
        "checksum123",
        vec![],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path)?;

    // Создаем symlist с абсолютным путем
    let symlist_path = pkg_dir.join("symlist");
    let target_dir = home_path.join("target-bin");
    std::fs::create_dir_all(&target_dir)?;
    std::fs::write(
        &symlist_path,
        &format!("bin/app {}", target_dir.join("app").display()),
    )?;

    // Create archive
    let archive_path = home_path.join("test-app.uhp");
    let tar_gz = std::fs::File::create(&archive_path)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    // Правильно добавляем файлы в архив
    tar.append_path_with_name(&meta_path, "uhp.toml")?;
    tar.append_path_with_name(&symlist_path, "symlist")?;
    tar.append_dir_all("bin", &bin_dir)?;

    tar.finish()?;

    // Install
    installer::install(&archive_path, &db).await?;

    // Verify installation
    let version = db.get_package_version("test-app").await?;
    assert_eq!(version, Some("1.0.0".to_string()));

    // Проверяем, что пакет установился
    let pkg_install_dir = home_path.join(".uhpm/packages/test-app-1.0.0");
    assert!(pkg_install_dir.exists(), "Package should be installed");

    // Remove
    remover::remove("test-app", &db).await?;

    let version_after = db.get_package_version("test-app").await?;
    assert!(
        version_after.is_none(),
        "Package should be removed from database"
    );

    // Проверяем, что директория пакета удалена
    assert!(
        !pkg_install_dir.exists(),
        "Package directory should be removed"
    );

    Ok(())
}

// Тест для проверки установки пакета с зависимостями
#[tokio::test]
async fn test_package_with_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = tempdir()?;
    let home_path = tmp_dir.path().to_path_buf();
    unsafe {
        std::env::set_var("HOME", &home_path);
    }

    std::fs::create_dir_all(home_path.join(".uhpm/packages"))?;

    let db_path = home_path.join(".uhpm/packages.db");
    let db = PackageDB::new(&db_path)?.init().await?;

    // Create package with dependencies
    let pkg_dir = home_path.join("dep-pkg");
    std::fs::create_dir_all(&pkg_dir)?;

    let pkg = Package::new(
        "package-with-deps",
        semver::Version::parse("1.0.0").unwrap(),
        "Test Author",
        Source::Raw("test://with-deps".to_string()),
        "checksum456",
        vec![
            (
                "dep-package-1".to_string(),
                semver::Version::parse("1.0.0").unwrap(),
            ),
            (
                "dep-package-2".to_string(),
                semver::Version::parse("2.0.0").unwrap(),
            ),
        ],
    );

    let meta_path = pkg_dir.join("uhp.toml");
    pkg.save_to_toml(&meta_path)?;

    // Create minimal symlist
    let symlist_path = pkg_dir.join("symlist");
    std::fs::write(&symlist_path, "# Empty symlist for test")?;

    // Create archive
    let archive_path = home_path.join("package-with-deps.uhp");
    let tar_gz = std::fs::File::create(&archive_path)?;
    let enc = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
    let mut tar = tar::Builder::new(enc);

    tar.append_path_with_name(&meta_path, "uhp.toml")?;
    tar.append_path_with_name(&symlist_path, "symlist")?;

    tar.finish()?;

    // Install
    installer::install(&archive_path, &db).await?;

    // Verify installation and dependencies
    let installed_pkg = db.get_current_package("package-with-deps").await?;
    assert!(installed_pkg.is_some(), "Package should be installed");

    let pkg = installed_pkg.unwrap();
    let deps = pkg.dependencies();
    assert_eq!(deps.len(), 2, "Should have 2 dependencies");
    assert_eq!(deps[0].0, "dep-package-1");
    assert_eq!(deps[1].0, "dep-package-2");

    // Cleanup
    remover::remove("package-with-deps", &db).await?;

    Ok(())
}
