use semver::Version;
use std::path::{Path, PathBuf};
use std::fs;
use thiserror::Error;
use ron::from_str;
use serde::{Serialize, Deserialize};
use ron::error::SpannedError;

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
    Raw(String)
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
    pub fn name(&self) -> &str { &self.name }
    pub fn author(&self) -> &str { &self.author }
    pub fn version(&self) -> &Version { &self.version }
    pub fn src(&self) -> &Source { &self.src }
    pub fn checksum(&self) -> &str { &self.checksum }
    pub fn dependencies(&self) -> &[(String, Version)] { &self.dependencies }
    pub fn from_ron_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path)?;
        let pkg = from_str(&data)?;
        Ok(pkg)
    }

}

pub fn meta_parser(meta_path: &Path) -> Result<Package,MetaParseError> {
    let data = fs::read_to_string(meta_path)?;

        // Парсим RON в Package
        let pkg: Package = ron::from_str(&data)?;

        Ok(pkg)
}
