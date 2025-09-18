//! # Mocks for UHPM Package System
//!
//! This module provides mock implementations of database, unpacker, and symlink creator
//! for testing UHPM package installation logic without touching real filesystem or database.

use crate::db::PackageDBTrait;
use crate::package::Package;
use crate::package::installer::{SymlinkCreatorTrait, UnpackerTrait};
use async_trait::async_trait;
use semver::Version;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

/// ----------------------
/// Mock Database
/// ----------------------
#[derive(Default, Clone)]
pub struct MockDB {
    pub versions: Arc<Mutex<HashMap<String, String>>>,
}

#[async_trait]
impl PackageDBTrait for MockDB {
    async fn is_installed(&self, name: &str) -> Result<Option<Version>, sqlx::Error> {
        let versions = self.versions.lock().await;
        Ok(versions.get(name).map(|v| Version::parse(v).unwrap()))
    }

    async fn add_package_full(&self, pkg: &Package, _files: &[String]) -> Result<(), sqlx::Error> {
        let mut versions = self.versions.lock().await;
        versions.insert(pkg.name().to_string(), pkg.version().to_string());
        Ok(())
    }

    async fn set_current_version(&self, name: &str, version: &str) -> Result<(), sqlx::Error> {
        let mut versions = self.versions.lock().await;
        versions.insert(name.to_string(), version.to_string());
        Ok(())
    }

    async fn get_installed_files(&self, _pkg_name: &str) -> Result<Vec<String>, sqlx::Error> {
        Ok(Vec::new())
    }
}

/// ----------------------
/// Mock Unpacker
/// ----------------------
#[derive(Clone)]
pub struct MockUnpacker {
    pub path: PathBuf,
}

#[async_trait]
impl UnpackerTrait for MockUnpacker {
    async fn unpack(&self, _pkg_path: &Path) -> Result<PathBuf, std::io::Error> {
        Ok(self.path.clone())
    }
}

/// ----------------------
/// Mock Symlink Creator
/// ----------------------
#[derive(Clone)]
pub struct MockSymlink {
    pub files: Vec<PathBuf>,
}

impl SymlinkCreatorTrait for MockSymlink {
    fn create_symlinks(&self, _package_root: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
        Ok(self.files.clone())
    }
}
