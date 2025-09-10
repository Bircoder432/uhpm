use ron::error::SpannedError;
use ron::from_str;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod installer;
pub mod remover;
pub mod switcher;
pub mod updater;

#[derive(Error, Debug)]
pub enum MetaParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RON parse error: {0}")]
    Ron(#[from] SpannedError),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Source {
    Url(String),
    LocalPath(PathBuf),
    Raw(String),
}

impl Source {
    pub fn as_str(&self) -> &str {
        match self {
            Source::Url(s) | Source::Raw(s) => s,
            Source::LocalPath(p) => p.to_str().unwrap_or_default(),
        }
    }
}

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
    pub fn dependencies(&self) -> &[(String, Version)] {
        &self.dependencies
    }
    pub fn from_ron_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path)?;
        let pkg = from_str(&data)?;
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
    pub fn save_to_ron(&self, path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        let pretty = ron::ser::PrettyConfig::new();
        let ron_str = ron::ser::to_string_pretty(self, pretty)?;
        std::fs::write(path, ron_str)?;
        Ok(())
    }
}

pub fn meta_parser(meta_path: &Path) -> Result<Package, MetaParseError> {
    let data = fs::read_to_string(meta_path)?;

    // Парсим RON в Package
    let pkg: Package = ron::from_str(&data)?;

    Ok(pkg)
}

pub fn get_pkg_path(pkg_name: &str, pkg_ver: Version) -> PathBuf {
    let packages_path: PathBuf = dirs::home_dir().unwrap().join(".uhpm").join("packages");
    return packages_path.join(format!("{}-{}", pkg_name, pkg_ver.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;
    use std::fs;
    use std::path::PathBuf;

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
