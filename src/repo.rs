//! # Repository Module
//!
//! This module defines [`RepoDB`] and related utilities for managing package
//! repositories in **UHPM (Universal Home Package Manager)**.
//!
//! ## Responsibilities
//! - Store metadata about available packages in a repository (SQLite).
//! - Provide URLs for downloading package archives.
//! - Parse repository configuration files (`repos.ron`).
//!
//! ## Tables
//! - **`packages`**
//!   - Stores basic package metadata: `name`, `version`, `author`, `src`, `checksum`.
//! - **`urls`**
//!   - Maps `(name, version)` pairs to download URLs.
//!
//! ## Example
//! ```rust,no_run
//! use uhpm::repo::RepoDB;
//! use std::path::Path;
//!
//! # tokio_test::block_on(async {
//! let repo_db = RepoDB::new(Path::new("/tmp/repo.db")).await.unwrap();
//!
//! // Add a package
//! repo_db.add_package("foo", "1.0.0", "Alice", "src", "sha256").await.unwrap();
//! repo_db.add_url("foo", "1.0.0", "https://example.com/foo-1.0.0.uhp").await.unwrap();
//!
//! // List packages
//! let pkgs = repo_db.list_packages().await.unwrap();
//! println!("{:?}", pkgs);
//! # });
//! ```

use ron::from_str;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors that may occur while working with repositories.
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

/// SQLite-backed package repository database.
///
/// Used for storing available package metadata and their download URLs.
pub struct RepoDB {
    pool: SqlitePool,
}

impl RepoDB {
    pub fn pool(&self) -> &SqlitePool {
        return &self.pool;
    }
    /// Opens (or creates) a new repository database at the given path.
    ///
    /// Ensures required tables exist by calling [`RepoDB::init_tables`].
    pub async fn new(db_path: &Path) -> Result<Self, sqlx::Error> {
        if !db_path.exists() {
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).expect("Failed to create directory for database");
            }
            std::fs::File::create(db_path).expect("Cannot create database file");
        }

        let db_url = format!("sqlite://{}", db_path.to_str().unwrap());
        let pool = SqlitePool::connect(&db_url).await?;
        let db = RepoDB { pool };
        db.init_tables().await?;
        Ok(db)
    }

    /// Initializes required tables (`packages`, `urls`) if they don’t exist.
    async fn init_tables(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS packages (
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                author TEXT NOT NULL,
                src TEXT NOT NULL,
                checksum TEXT NOT NULL,
                PRIMARY KEY(name, version)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS urls (
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                url TEXT NOT NULL,
                PRIMARY KEY(name, version)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Returns the download URL for a given package name and version.
    ///
    /// # Errors
    /// Returns [`RepoError::NotFound`] if no URL is found.
    pub async fn get_package(&self, name: &str, version: &str) -> Result<String, RepoError> {
        let row = sqlx::query("SELECT url FROM urls WHERE name = ? AND version = ?")
            .bind(name)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(r.get::<String, _>("url")),
            None => Err(RepoError::NotFound(format!("{}-{}", name, version))),
        }
    }

    /// Lists all packages (name and version) available in this repository.
    pub async fn list_packages(&self) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT name, version FROM packages")
            .fetch_all(&self.pool)
            .await?;

        let packages = rows
            .into_iter()
            .map(|r| (r.get::<String, _>("name"), r.get::<String, _>("version")))
            .collect();

        Ok(packages)
    }

    /// Adds a package record to the repository (metadata only).
    pub async fn add_package(
        &self,
        name: &str,
        version: &str,
        author: &str,
        src: &str,
        checksum: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR IGNORE INTO packages (name, version, author, src, checksum) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(name)
        .bind(version)
        .bind(author)
        .bind(src)
        .bind(checksum)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Adds or replaces a download URL for a package version.
    pub async fn add_url(&self, name: &str, version: &str, url: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO urls (name, version, url) VALUES (?, ?, ?)")
            .bind(name)
            .bind(version)
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Parses a `.ron` repositories configuration file.
///
/// The file should define a map of repository names to paths/URLs.
/// Example (`repos.ron`):
/// ```ron
/// {
///     "local": "file:///home/user/uhpm-repo",
///     "main": "https://example.com/uhpm-repo"
/// }
/// ```
pub fn parse_repos<P: AsRef<Path>>(path: P) -> Result<HashMap<String, String>, RepoError> {
    let content = fs::read_to_string(path)?;
    let repos: HashMap<String, String> = from_str(&content).unwrap();
    Ok(repos)
}
