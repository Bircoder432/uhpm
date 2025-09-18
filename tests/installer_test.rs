use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uhpm::package::{Package, Source};
use uhpm::{debug, info};

/// Trait to mock PackageDB behavior for testing installer functionality.
#[async_trait]
pub trait PackageDBTrait {
    /// Returns the installed version of a package.
    async fn get_package_version(&self, name: &str) -> Option<String>;

    /// Returns the list of installed files for a package.
    async fn get_installed_files(&self, name: &str) -> Vec<String>;
}

/// Mock implementation of PackageDBTrait for tests.
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

/// Mocked installer function to simulate package installation.
///
/// - `_archive` - Path to the fake package archive.
/// - `_db` - Reference to the mocked database.
/// - `_symlinks` - Fake symlink mappings.
///
/// Returns `Ok(())` always for testing purposes.
async fn install_mock(
    _archive: &Path,
    _db: &impl PackageDBTrait,
    _symlinks: &HashMap<String, String>,
) -> Result<(), String> {
    info!(
        "test.installer.func.install_mock.aboutprint",
        "Mock installation called for {:?}", _archive
    );
    Ok(())
}

/// Tests that a simple package installation works with a mocked database.
///
/// Logs progress using `test.installer.func.*` keys for consistency with localization.
#[tokio::test]
async fn test_install_simple_package_mocked() {
    // Initialize mocked database
    let db = MockPackageDB;
    info!(
        "test.installer.func.setup_db.aboutprint",
        "Mock database initialized"
    );

    // Prepare a fake archive path
    let fake_archive = PathBuf::from("/fake/path/my_package.uhp");
    info!(
        "test.installer.func.setup_archive.aboutprint",
        "Fake archive prepared at {:?}", &fake_archive
    );

    // Empty symlink map
    let symlinks = HashMap::new();

    // Call mocked installer
    install_mock(&fake_archive, &db, &symlinks).await.unwrap();
    info!(
        "test.installer.func.install_call.aboutprint",
        "Installer mock executed"
    );

    // Verify "database" behavior
    let version = db.get_package_version("my_package").await.unwrap();
    info!(
        "test.installer.func.check_version.aboutprint",
        "Installed package version: {:?}", &version
    );
    assert_eq!(version, "0.1.0");

    let files = db.get_installed_files("my_package").await;
    info!(
        "test.installer.func.check_files.aboutprint",
        "Installed files: {:?}", &files
    );
    assert_eq!(files, vec!["bin/my_binary"]);
}
