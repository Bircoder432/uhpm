use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Builder;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, installer};
use uhpm::{debug, info};

/// Recursively adds a directory and all its contents to a tar.gz archive.
///
/// # Arguments
/// * `tar` - A mutable reference to the tar builder.
/// * `path` - Path of the directory to add.
/// * `base` - Base path for computing relative paths in the archive.
///
/// # Logs
/// Uses `debug!("test.switcher.funcname.aboutprint", relative_path)` to log each file added.
fn append_dir_all(tar: &mut Builder<GzEncoder<File>>, path: &Path, base: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap();
        if path.is_dir() {
            append_dir_all(tar, &path, base);
        } else {
            tar.append_path_with_name(&path, rel_path).unwrap();
            debug!(
                "test.switcher.append_dir_all.file_added_to_archive",
                rel_path
            );
        }
    }
}

/// Tests installing a simple package archive (.uhp) using the installer.
///
/// Steps performed:
/// 1. Creates a temporary directory for HOME and package structure.
/// 2. Generates `uhp.ron` and `symlist.ron`.
/// 3. Packs the package into a `.uhp` tar.gz archive.
/// 4. Initializes an in-memory PackageDB.
/// 5. Installs the package using `installer::install`.
/// 6. Asserts that the version and installed files are correctly recorded.
///
/// # Logs
/// Uses `info!("test.switcher.test_install_simple_package.aboutprint", ...)` for key steps.
#[tokio::test]
async fn test_install_simple_package() {
    let _ = tracing_subscriber::fmt().try_init();

    let tmp_dir = tempdir().unwrap();
    info!(
        "test.switcher.test_install_simple_package.tmp_dir",
        tmp_dir.path()
    );

    // Redirect HOME
    unsafe { std::env::set_var("HOME", tmp_dir.path()) };
    info!(
        "test.switcher.test_install_simple_package.home_redirected",
        tmp_dir.path()
    );

    // Create package structure
    let pkg_dir = tmp_dir.path().join("pkg_contents");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_file = bin_dir.join("my_binary");
    fs::write(&bin_file, "#!/bin/bash\necho hello").unwrap();
    info!(
        "test.switcher.test_install_simple_package.binary_created",
        bin_file
    );

    // Generate uhp.ron
    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();
    info!(
        "test.switcher.test_install_simple_package.ron_generated",
        meta_path
    );

    // Generate symlist.ron
    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[
    (source: "bin/my_binary", target: "$HOME/.local/bin/my_binary")
]"#,
    )
    .unwrap();
    info!(
        "test.switcher.test_install_simple_package.symlist_generated",
        symlist_path
    );

    // Create installation folder
    fs::create_dir_all(tmp_dir.path().join(".local/bin")).unwrap();

    // Pack the archive
    let uhp_path = tmp_dir.path().join("my_package.uhp");
    let tar_gz = File::create(&uhp_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    append_dir_all(&mut tar, &pkg_dir, &pkg_dir);
    tar.finish().unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    info!(
        "test.switcher.test_install_simple_package.archive_created",
        &uhp_path
    );

    // Initialize database
    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();
    info!(
        "test.switcher.test_install_simple_package.db_initialized",
        &db_path
    );

    // Install package
    installer::install(&uhp_path, &db).await.unwrap();
    info!("test.switcher.test_install_simple_package.installation_complete");

    // Verify version
    let version = db.get_package_version("my_package").await.unwrap();
    info!(
        "test.switcher.test_install_simple_package.package_version",
        &version
    );
    assert!(version.is_some(), "Package not added to database");
    assert_eq!(version.unwrap(), "0.1.0");

    // Verify installed files
    let installed_files = db.get_installed_files("my_package").await.unwrap();
    info!(
        "test.switcher.test_install_simple_package.installed_files",
        &installed_files
    );
    assert!(!installed_files.is_empty(), "Files not installed");
}
