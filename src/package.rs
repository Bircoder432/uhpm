//! # Package Module
//!
//! This module defines the core data structures and utilities used for
//! representing and managing UHPM packages.
//!
//! ## Responsibilities
//! - Define the [`Package`] structure (name, version, author, source, checksum, dependencies).
//! - Represent package sources via the [`Source`] enum.
//! - Parse and serialize package metadata from/to `.ron` files.
//! - Provide helper functions like [`meta_parser`] and [`get_pkg_path`].
//!
//! ## Submodules
//! - [`installer`] — Package installation logic.
//! - [`remover`] — Package removal logic.
//! - [`switcher`] — Switching between package versions.
//! - [`updater`] — Updating packages to newer versions.
//!
//! ## Example
//! ```rust,no_run
//! use uhpm::package::{Package, Source};
//! use semver::Version;
//!
//! let pkg = Package::new(
//!     "hello",
//!     Version::parse("1.0.0").unwrap(),
//!     "Alice",
//!     Source::Raw("http://example.com/hello-1.0.0.uhp".to_string()),
//!     "sha256sum",
//!     vec![]
//! );
//!
//! println!("Package {} v{}", pkg.name(), pkg.version());
//! ```

use crate::error::MetaParseError;
use ron::from_str;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
pub mod installer;
pub mod remover;
pub mod switcher;
pub mod updater;

/// Errors that may occur while parsing package metadata.

/// Represents the source of a package.
///
/// A package can originate from:
/// - A remote URL
/// - A local file path
/// - An inline/raw string
#[derive(Serialize, Deserialize, Debug)]
pub enum Source {
    Url(String),
    LocalPath(PathBuf),
    Raw(String),
}

impl Source {
    /// Returns the source as a string slice.
    ///
    /// For `LocalPath`, attempts to convert the path to a UTF-8 string.
    pub fn as_str(&self) -> &str {
        match self {
            Source::Url(s) | Source::Raw(s) => s,
            Source::LocalPath(p) => p.to_str().unwrap_or_default(),
        }
    }
}

/// Represents a UHPM package with its metadata and dependencies.
#[derive(Serialize, Deserialize, Debug)]
pub struct Package {
    name: String,
    author: String,
    version: Version,
    src: Source,
    checksum: String,
    dependencies: Vec<(String, Version)>,
}

impl Package {
    /// Creates a new [`Package`] instance.
    pub fn new(
        name: impl Into<String>,
        version: Version,
        author: impl Into<String>,
        src: Source,
        checksum: impl Into<String>,
        dependencies: Vec<(String, Version)>,
    ) -> Self {
        Self {
            name: name.into(),
            version,
            author: author.into(),
            src,
            checksum: checksum.into(),
            dependencies,
        }
    }

    /// Returns the package name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the author of the package.
    pub fn author(&self) -> &str {
        &self.author
    }

    /// Returns the package version.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Returns the source of the package.
    pub fn src(&self) -> &Source {
        &self.src
    }

    /// Returns the checksum string of the package.
    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    /// Returns the package dependencies as a slice of `(name, version)` pairs.
    pub fn dependencies(&self) -> &[(String, Version)] {
        &self.dependencies
    }

    /// Loads a package definition from a `.ron` file.
    pub fn from_ron_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path)?;
        let pkg = from_str(&data)?;
        Ok(pkg)
    }

    /// Returns a template package definition, useful for generating a starting point.
    pub fn template() -> Self {
        Package {
            name: "my_package".to_string(),
            author: "YourName".to_string(),
            version: Version::parse("0.1.0").unwrap(),
            src: Source::Raw("TODO".to_string()),
            checksum: "TODO".to_string(),
            dependencies: vec![],
        }
    }

    /// Saves the package definition to a `.ron` file.
    pub fn save_to_ron(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let pretty = ron::ser::PrettyConfig::new();
        let ron_str = ron::ser::to_string_pretty(self, pretty)?;
        std::fs::write(path, ron_str)?;
        Ok(())
    }
}

/// Parses a `.ron` metadata file into a [`Package`].
///
/// # Errors
/// Returns [`MetaParseError`] if the file cannot be read or parsed.
pub fn meta_parser(meta_path: &Path) -> Result<Package, MetaParseError> {
    let data = fs::read_to_string(meta_path)?;
    let pkg: Package = ron::from_str(&data)?;
    Ok(pkg)
}

/// Returns the expected installation path for a package.
pub fn get_pkg_path(pkg_name: &str, pkg_ver: Version) -> PathBuf {
    let packages_path: PathBuf = dirs::home_dir().unwrap().join(".uhpm").join("packages");
    packages_path.join(format!("{}-{}", pkg_name, pkg_ver.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use std::fs;

    fn sample_package_ron() -> String {
        r#"
            Package(
                name: "test_pkg",
                author: "Tester",
                version: "0.1.0",
                src: Raw("some content"),
                checksum: "abc123",
                dependencies: []
            )
            "#
        .to_string()
    }

    #[test]
    fn test_from_ron_file() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let ron_path = tmp_dir.path().join("uhp.ron");
        fs::write(&ron_path, sample_package_ron()).unwrap();

        let pkg = Package::from_ron_file(&ron_path).unwrap();
        assert_eq!(pkg.name(), "test_pkg");
        assert_eq!(pkg.author(), "Tester");
        assert_eq!(pkg.version(), &Version::parse("0.1.0").unwrap());
        assert_eq!(pkg.src().as_str(), "some content");
        assert_eq!(pkg.checksum(), "abc123");
        assert!(pkg.dependencies().is_empty());
    }

    #[test]
    fn test_meta_parser() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let ron_path = tmp_dir.path().join("uhp.ron");
        fs::write(&ron_path, sample_package_ron()).unwrap();

        let pkg = meta_parser(&ron_path).unwrap();
        assert_eq!(pkg.name(), "test_pkg");
        assert_eq!(pkg.author(), "Tester");
    }
}
