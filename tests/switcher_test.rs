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

    // Redirect HOME
    unsafe { std::env::set_var("HOME", tmp_dir.path()) };

    // Create package structure
    let pkg_dir = tmp_dir.path().join("pkg_contents");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_file = bin_dir.join("my_binary");
    fs::write(&bin_file, "#!/bin/bash\necho hello").unwrap();

    // Generate uhp.ron
    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();

    // Generate symlist.ron
    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[
    (source: "bin/my_binary", target: "$HOME/.local/bin/my_binary")
]"#,
    )
    .unwrap();

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

    // Initialize database
    let db_path = tmp_dir.path().join("packages.db");
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    // Install package
    installer::install(&uhp_path, &db).await.unwrap();

    // Verify version
    let version = db.get_package_version("my_package").await.unwrap();

    assert!(version.is_some(), "Package not added to database");
    assert_eq!(version.unwrap(), "0.1.0");

    // Verify installed files
    let installed_files = db.get_installed_files("my_package").await.unwrap();

    assert!(!installed_files.is_empty(), "Files not installed");
}
