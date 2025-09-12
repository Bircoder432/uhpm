use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{self, File};
use std::path::Path;
use tar::Builder;
use tempfile::tempdir;
use uhpm::db::PackageDB;
use uhpm::package::{Package, installer};
use uhpm::{debug, info};

fn append_dir_all(tar: &mut Builder<GzEncoder<File>>, path: &Path, base: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel_path = path.strip_prefix(base).unwrap();
        if path.is_dir() {
            append_dir_all(tar, &path, base);
        } else {
            tar.append_path_with_name(&path, rel_path).unwrap();
            debug!("test.installer.file_added_to_archive", rel_path);
        }
    }
}

#[tokio::test]
async fn test_install_simple_package() {
    let _ = tracing_subscriber::fmt().try_init();

    let tmp_dir = tempdir().unwrap();
    info!("test.installer.tmp_dir", tmp_dir.path());

    unsafe {
        std::env::set_var("HOME", tmp_dir.path());
    }
    info!("test.installer.home_redirected", tmp_dir.path());

    let pkg_dir = tmp_dir.path().join("pkg_contents");
    fs::create_dir_all(&pkg_dir).unwrap();

    let bin_dir = pkg_dir.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let bin_file = bin_dir.join("my_binary");
    fs::write(&bin_file, "#!/bin/bash\necho hello").unwrap();
    info!("test.installer.binary_created", bin_file);

    let pkg = Package::template();
    let meta_path = pkg_dir.join("uhp.ron");
    pkg.save_to_ron(&meta_path).unwrap();
    let ron_content = fs::read_to_string(&meta_path).unwrap();
    info!("test.installer.ron_generated", ron_content);

    let symlist_path = pkg_dir.join("symlist.ron");
    fs::write(
        &symlist_path,
        r#"[
    (source: "bin/my_binary", target: "$HOME/.local/bin/my_binary")
]"#,
    )
    .unwrap();
    let symlist_content = fs::read_to_string(&symlist_path).unwrap();
    info!("test.installer.symlist_generated", symlist_content);

    fs::create_dir_all(tmp_dir.path().join(".local/bin")).unwrap();

    let uhp_path = tmp_dir.path().join("my_package.uhp");
    info!("test.installer.archive_creation", &uhp_path);
    let tar_gz = File::create(&uhp_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = Builder::new(enc);
    append_dir_all(&mut tar, &pkg_dir, &pkg_dir);
    tar.finish().unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    info!("test.installer.archive_created", &uhp_path);

    let db_path = tmp_dir.path().join("packages.db");
    info!("test.installer.db_creation", &db_path);
    let db = PackageDB::new(&db_path).unwrap().init().await.unwrap();

    info!("test.installer.installation_start");
    installer::install(&uhp_path, &db).await.unwrap();
    info!("test.installer.installation_complete");

    let version = db.get_package_version("my_package").await.unwrap();
    info!("test.installer.package_version", &version);
    assert!(version.is_some(), "Пакет не добавлен в базу!");
    assert_eq!(version.unwrap(), "0.1.0");

    let installed_files = db.get_installed_files("my_package").await.unwrap();
    info!("test.installer.installed_files", &installed_files);
    assert!(!installed_files.is_empty(), "Файлы пакета не добавлены!");
}
