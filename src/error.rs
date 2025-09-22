use ron::error::SpannedError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FetchError {
    /// HTTP or network-related error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error reported by the installer.
    #[error("Installer error: {0}")]
    Installer(String),
}

#[derive(Error, Debug)]
pub enum MetaParseError {
    /// Filesystem error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parsing error when reading a `.ron` file.
    #[error("RON parse error: {0}")]
    Ron(#[from] SpannedError),
}

#[derive(Error, Debug)]
pub enum UpdaterError {
    /// The package is not installed and therefore cannot be updated.
    #[error("Package not found: {0}")]
    NotFound(String),

    /// Filesystem or I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Error while working with repository database or configuration.
    #[error("Repo error: {0}")]
    Repo(#[from] RepoError),

    /// Database error from `sqlx`.
    #[error("DB error: {0}")]
    Db(#[from] sqlx::Error),

    /// Error during fetch or installation of the new package.
    #[error("Fetch error: {0}")]
    Fetch(#[from] crate::error::FetchError),

    /// No newer version available
    #[error("No newer version available for package: {0}")]
    NoNewVersion(String),
}

#[derive(Error, Debug)]
pub enum RepoError {
    /// Filesystem error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// SQLite error.
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    /// Package not found in the repository.
    #[error("Package not found: {0}")]
    NotFound(String),
}

#[derive(Error, Debug)]
pub enum PackerError {
    #[error("Config error: {0}")]
    Config(String),

    #[error("Build error: {0}")]
    Build(String),

    #[error("Install error: {0}")]
    Install(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
