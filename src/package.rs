//! # Package Module

use crate::error::MetaParseError;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
pub mod installer;
pub mod remover;
pub mod switcher;
pub mod updater;

/// Represents the source of a package.
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type", content = "value")]
pub enum Source {
    Url(String),
    LocalPath(String),
    Raw(String),
}

impl Source {
    pub fn as_str(&self) -> &str {
        match self {
            Source::Url(s) | Source::Raw(s) => s,
            Source::LocalPath(p) => p,
        }
    }
}

/// Represents a dependency with name and version
#[derive(Serialize, Deserialize, Debug)]
pub struct Dependency {
    pub name: String,
    pub version: Version,
}

/// Represents a UHPM package with its metadata and dependencies.
#[derive(Serialize, Deserialize, Debug)]
pub struct Package {
    name: String,
    author: String,
    version: Version,
    src: Source,
    checksum: String,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

impl Package {
    pub fn new(
        name: impl Into<String>,
        version: Version,
        author: impl Into<String>,
        src: Source,
        checksum: impl Into<String>,
        dependencies: Vec<(String, Version)>,
    ) -> Self {
        let deps = dependencies
            .into_iter()
            .map(|(name, version)| Dependency { name, version })
            .collect();

        Self {
            name: name.into(),
            version,
            author: author.into(),
            src,
            checksum: checksum.into(),
            dependencies: deps,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn author(&self) -> &str {
        &self.author
    }

    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn src(&self) -> &Source {
        &self.src
    }

    pub fn checksum(&self) -> &str {
        &self.checksum
    }

    pub fn dependencies(&self) -> Vec<(String, Version)> {
        self.dependencies
            .iter()
            .map(|dep| (dep.name.clone(), dep.version.clone()))
            .collect()
    }

    pub fn from_toml_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path)?;
        let pkg: Package = toml::from_str(&data)?;
        Ok(pkg)
    }

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

    pub fn save_to_toml(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let toml_str = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }
}

pub fn meta_parser(meta_path: &Path) -> Result<Package, MetaParseError> {
    let data = fs::read_to_string(meta_path)?;
    let pkg: Package = toml::from_str(&data).map_err(|e| {
        MetaParseError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("TOML parse error: {}", e),
        ))
    })?;
    Ok(pkg)
}

pub fn get_pkg_path(pkg_name: &str, pkg_ver: Version) -> PathBuf {
    let packages_path: PathBuf = dirs::home_dir().unwrap().join(".uhpm").join("packages");
    packages_path.join(format!("{}-{}", pkg_name, pkg_ver.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use std::fs;

    fn sample_package_toml() -> String {
        r#"
name = "test_pkg"
author = "Tester"
version = "0.1.0"
checksum = "abc123"

[src]
type = "Raw"
value = "some content"

[[dependencies]]
name = "dep_pkg"
version = "1.0.0"
"#
        .to_string()
    }

    #[test]
    fn test_from_toml_file() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let toml_path = tmp_dir.path().join("uhp.toml");
        fs::write(&toml_path, sample_package_toml()).unwrap();

        let pkg = Package::from_toml_file(&toml_path).unwrap();
        assert_eq!(pkg.name(), "test_pkg");
        assert_eq!(pkg.author(), "Tester");
        assert_eq!(pkg.version(), &Version::parse("0.1.0").unwrap());
        assert_eq!(pkg.src().as_str(), "some content");
        assert_eq!(pkg.checksum(), "abc123");
        assert_eq!(pkg.dependencies().len(), 1);
        assert_eq!(pkg.dependencies()[0].0, "dep_pkg");
        assert_eq!(pkg.dependencies()[0].1, Version::parse("1.0.0").unwrap());
    }

    #[test]
    fn test_meta_parser() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let toml_path = tmp_dir.path().join("uhp.toml");
        fs::write(&toml_path, sample_package_toml()).unwrap();

        let pkg = meta_parser(&toml_path).unwrap();
        assert_eq!(pkg.name(), "test_pkg");
        assert_eq!(pkg.author(), "Tester");
    }

    #[test]
    fn test_save_and_load_toml() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let toml_path = tmp_dir.path().join("test_pkg.toml");

        let original_pkg = Package::new(
            "test_package",
            Version::parse("1.2.3").unwrap(),
            "Test Author",
            Source::Url("https://example.com/pkg.uhp".to_string()),
            "sha256:abc123",
            vec![
                ("dep1".to_string(), Version::parse("1.0.0").unwrap()),
                ("dep2".to_string(), Version::parse("2.0.0").unwrap()),
            ],
        );

        original_pkg.save_to_toml(&toml_path).unwrap();

        // Проверим, что сохранилось правильно
        let saved_content = fs::read_to_string(&toml_path).unwrap();
        println!("Saved TOML:\n{}", saved_content);

        let loaded_pkg = Package::from_toml_file(&toml_path).unwrap();

        assert_eq!(original_pkg.name(), loaded_pkg.name());
        assert_eq!(original_pkg.author(), loaded_pkg.author());
        assert_eq!(original_pkg.version(), loaded_pkg.version());
        assert_eq!(original_pkg.checksum(), loaded_pkg.checksum());
        assert_eq!(
            original_pkg.dependencies().len(),
            loaded_pkg.dependencies().len()
        );
    }

    #[test]
    fn test_source_serialization() {
        let pkg = Package::new(
            "test",
            Version::parse("1.0.0").unwrap(),
            "author",
            Source::Raw("content".to_string()),
            "checksum",
            vec![],
        );

        let toml_str = toml::to_string_pretty(&pkg).unwrap();
        println!("Serialized package:\n{}", toml_str);

        // Проверим, что можем десериализовать обратно
        let deserialized: Package = toml::from_str(&toml_str).unwrap();
        assert_eq!(pkg.name(), deserialized.name());
    }
}
