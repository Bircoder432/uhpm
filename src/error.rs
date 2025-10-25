use ron::error::SpannedError;
use semver::Version;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SwitchError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Package directory not found: {0}")]
    MissingPackageDir(PathBuf),
    #[error("Symlist error: {0}")]
    Symlist(#[from] crate::symlist::SymlistError),
    #[error("Package not found: {0} version {1}")]
    PackageNotFound(String, Version),
}

#[derive(Error, Debug)]
pub enum UpdaterError {
    #[error("Package not found: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Repository error: {0}")]
    Repo(#[from] RepoError),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Fetch error: {0}")]
    Fetch(#[from] FetchError),
    #[error("No newer version available for package: {0}")]
    NoNewVersion(String),
}

#[derive(Error, Debug)]
pub enum UhpmError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Repository error: {0}")]
    Repository(#[from] RepoError),
    #[error("Package error: {0}")]
    Package(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Package not found: {0}")]
    NotFound(String),
    #[error("No newer version available for package: {0}")]
    NoNewVersion(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Error, Debug)]
pub enum RemoveError {
    #[error("Package not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Ron(#[from] SpannedError),
    #[error("Configuration file not found: {0}")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum RepoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Package not found: {0}")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum FetchError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Installer error: {0}")]
    Installer(String),
}

#[derive(Error, Debug)]
pub enum MetaParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Ron(#[from] SpannedError),
}

impl From<SwitchError> for UhpmError {
    fn from(error: SwitchError) -> Self {
        match error {
            SwitchError::Io(e) => UhpmError::Io(e),
            SwitchError::Db(e) => UhpmError::Database(e),
            SwitchError::MissingPackageDir(path) => {
                UhpmError::NotFound(format!("Package directory not found: {}", path.display()))
            }
            SwitchError::Symlist(e) => UhpmError::Parse(e.to_string()),
            SwitchError::PackageNotFound(name, version) => {
                UhpmError::NotFound(format!("Package {} version {} not found", name, version))
            }
        }
    }
}

impl From<UpdaterError> for UhpmError {
    fn from(error: UpdaterError) -> Self {
        match error {
            UpdaterError::NotFound(name) => UhpmError::NotFound(name),
            UpdaterError::Io(e) => UhpmError::Io(e),
            UpdaterError::Repo(e) => UhpmError::Repository(e),
            UpdaterError::Db(e) => UhpmError::Database(e),
            UpdaterError::Fetch(e) => UhpmError::from(e),
            UpdaterError::NoNewVersion(name) => UhpmError::NoNewVersion(name),
        }
    }
}

impl From<FetchError> for UhpmError {
    fn from(error: FetchError) -> Self {
        match error {
            FetchError::Http(e) => UhpmError::Network(e),
            FetchError::Io(e) => UhpmError::Io(e),
            FetchError::Installer(msg) => UhpmError::Package(msg),
        }
    }
}

impl From<MetaParseError> for UhpmError {
    fn from(error: MetaParseError) -> Self {
        match error {
            MetaParseError::Io(e) => UhpmError::Io(e),
            MetaParseError::Ron(e) => UhpmError::Parse(e.to_string()),
        }
    }
}

impl From<RemoveError> for UhpmError {
    fn from(error: RemoveError) -> Self {
        match error {
            RemoveError::NotFound(name) => UhpmError::NotFound(name),
            RemoveError::Db(e) => UhpmError::Database(e),
            RemoveError::Io(e) => UhpmError::Io(e),
        }
    }
}

impl From<String> for UhpmError {
    fn from(error: String) -> Self {
        UhpmError::Package(error)
    }
}

impl From<&str> for ConfigError {
    fn from(s: &str) -> Self {
        ConfigError::NotFound(s.to_string())
    }
}
