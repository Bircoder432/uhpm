use crate::db::PackageDB;
use crate::package::Package;
use crate::symlist;
use crate::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum InstallError {
    Io(std::io::Error),
    Meta(crate::package::MetaParseError),
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        InstallError::Io(e)
    }
}

impl From<crate::package::MetaParseError> for InstallError {
    fn from(e: crate::package::MetaParseError) -> Self {
        InstallError::Meta(e)
    }
}

pub async fn install(pkg_path: &Path, db: &PackageDB) -> Result<(), InstallError> {
    info!("installer.install.starting", pkg_path.display());

    let unpacked = unpack(pkg_path)?;
    debug!("installer.install.unpacked", unpacked.display());

    let meta_path = unpacked.join("uhp.ron");
    debug!("installer.install.reading_meta", meta_path.display());
    let package_meta: Package = crate::package::meta_parser(&meta_path)?;
    info!(
        "installer.install.package_info",
        package_meta.name(),
        package_meta.version()
    );

    let pkg_name = package_meta.name();
    let version = package_meta.version();

    let already_installed = db.is_installed(pkg_name).await.unwrap();
    if let Some(installed_version) = &already_installed {
        info!(
            "installer.install.already_installed",
            pkg_name, installed_version
        );
        if installed_version == version {
            info!("installer.install.same_version_skipped");
            return Ok(());
        }
    }

    let package_root = dirs::home_dir()
        .unwrap()
        .join(".uhpm/packages")
        .join(format!("{}-{}", pkg_name, version));
    debug!("installer.install.package_root", package_root.display());

    if package_root.exists() {
        debug!(
            "installer.install.removing_existing",
            package_root.display()
        );
        fs::remove_dir_all(&package_root)?;
    }
    fs::create_dir_all(&package_root)?;
    debug!("installer.install.created_dir", package_root.display());

    fs::rename(&unpacked, &package_root)?;
    debug!("installer.install.moved_package", package_root.display());

    let mut installed_files = Vec::new();
    match already_installed {
        None => {
            info!("installer.install.creating_symlinks");
            installed_files = create_symlinks(&package_root)?;
        }
        Some(_) => {
            info!("installer.install.updating_version");
        }
    }

    let installed_files_str: Vec<String> = installed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    info!(
        "installer.install.adding_to_db",
        pkg_name,
        installed_files_str.len()
    );
    db.add_package_full(&package_meta, &installed_files_str)
        .await
        .unwrap();
    db.set_current_version(&package_meta.name(), &package_meta.version().to_string())
        .await
        .unwrap();

    info!("installer.install.success", pkg_name);
    Ok(())
}

pub fn create_symlinks(package_root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut installed_files = Vec::new();

    let symlist_path = package_root.join("symlist.ron");
    debug!("installer.symlinks.loading", symlist_path.display());

    match symlist::load_symlist(&symlist_path, &package_root) {
        Ok(symlinks) => {
            for (src_rel, dst_abs) in symlinks {
                let src_abs = package_root.join(&src_rel);
                debug!(
                    "installer.symlinks.processing",
                    src_abs.display(),
                    dst_abs.display()
                );

                if !src_abs.exists() {
                    warn!("installer.symlinks.src_not_found", src_abs.display());
                    continue;
                }

                if let Some(parent) = dst_abs.parent() {
                    fs::create_dir_all(parent)?;
                    debug!("installer.symlinks.created_parent", parent.display());
                }

                if dst_abs.exists() {
                    fs::remove_file(&dst_abs)?;
                    debug!("installer.symlinks.removed_existing", dst_abs.display());
                }

                std::os::unix::fs::symlink(&src_abs, &dst_abs)?;
                debug!(
                    "installer.symlinks.created_link",
                    dst_abs.display(),
                    src_abs.display()
                );
                installed_files.push(dst_abs);
            }
        }
        Err(e) => {
            warn!("installer.symlinks.load_failed", e);
        }
    }

    debug!("installer.symlinks.total_created", installed_files.len());
    Ok(installed_files)
}

fn unpack(pkg_path: &Path) -> Result<PathBuf, std::io::Error> {
    if pkg_path.extension().and_then(|s| s.to_str()) != Some("uhp") {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Package must have .uhp extension",
        ));
    }

    let tmp_dir = dirs::home_dir().unwrap().join(".uhpm/tmp");
    fs::create_dir_all(&tmp_dir)?;

    let package_name = pkg_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown_package");
    let unpack_dir = tmp_dir.join(package_name);

    if unpack_dir.exists() {
        fs::remove_dir_all(&unpack_dir)?;
    }
    fs::create_dir_all(&unpack_dir)?;

    debug!(
        "installer.unpack.unpacking",
        pkg_path.display(),
        unpack_dir.display()
    );

    let tar_gz = fs::File::open(pkg_path)?;
    let decompressor = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(decompressor);
    archive.unpack(&unpack_dir)?;

    debug!("installer.unpack.done", unpack_dir.display());
    Ok(unpack_dir)
}
