//! # Package Database (`PackageDB`)
//!
//! This module provides an abstraction layer over an SQLite database used
//! by **UHPM (Universal Home Package Manager)** to track installed packages,
//! their versions, installed files, and dependencies.
//!
//! ## Responsibilities
//! - Initialize and maintain the SQLite database schema.
//! - Add, update, and remove package records.
//! - Track installed files and dependencies.
//! - Query package information, including versions and current package state.
//!
//! ## Tables
//! - **`packages`**
//!   - Stores package metadata (name, version, author, source, checksum).
//!   - Marks which version is currently active via the `current` column.
//!
//! - **`installed_files`**
//!   - Maps installed package files to their owning package.
//!
//! - **`dependencies`**
//!   - Tracks package dependencies by name and version.
//!
//! ## Example
//! ```rust,no_run
//! use uhpm::db::PackageDB;
//! use std::path::Path;
//!
//! # tokio_test::block_on(async {
//! let db = PackageDB::new(Path::new("/tmp/uhpm.db"))
//!     .unwrap()
//!     .init()
//!     .await
//!     .unwrap();
//!
//! let packages = db.list_packages().await.unwrap();
//! println!("Installed packages: {:?}", packages);
//! # });
//! ```

use crate::package::{Package, Source};
use crate::{debug, info};
use semver::Version;
use sqlx::Row;
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};

/// Represents the UHPM package database.
///
/// Internally, this is an SQLite database stored on disk,
/// providing structured access to package metadata.
pub struct PackageDB {
    pool: SqlitePool,
    path: PathBuf,
}

impl PackageDB {
    /// Creates a new `PackageDB` instance and ensures the database file exists.
    ///
    /// This does **not** establish a connection yet.
    ///
    /// # Arguments
    /// - `path`: Path to the SQLite database file.
    ///
    /// # Errors
    /// Returns [`std::io::Error`] if the file or directories cannot be created.
    pub fn new(path: &Path) -> Result<Self, std::io::Error> {
        debug!("db.new.creating", path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            std::fs::File::create(path)?;
            debug!("db.new.file_created", path);
        }

        // Placeholder pool, replaced later in `init`
        Ok(PackageDB {
            pool: SqlitePool::connect_lazy("sqlite::memory:")
                .expect("lazy pool must work for placeholder"),
            path: path.to_path_buf(),
        })
    }

    /// Establishes a real database connection and initializes tables if needed.
    ///
    /// # Errors
    /// Returns [`sqlx::Error`] if the database connection or table creation fails.
    pub async fn init(mut self) -> Result<Self, sqlx::Error> {
        let path_str = self.path.to_str().expect("Invalid UTF-8 path");
        let db_url = format!("sqlite://{}", path_str);
        debug!("db.init.connecting", &db_url);

        self.pool = SqlitePool::connect(&db_url).await?;

        debug!("db.init.ensuring_tables");
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                author TEXT NOT NULL,
                src TEXT NOT NULL,
                checksum TEXT NOT NULL,
                current BOOLEAN NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS installed_files (
                package_name TEXT NOT NULL,
                file_path TEXT NOT NULL,
                PRIMARY KEY(package_name, file_path)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dependencies (
                package_name TEXT NOT NULL,
                dependency_name TEXT NOT NULL,
                dependency_version TEXT NOT NULL,
                PRIMARY KEY(package_name, dependency_name)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        info!("db.init.success", &self.path);
        Ok(self)
    }

    /// Returns a reference to the connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Adds or replaces a package entry in the database (without files or dependencies).
    pub async fn add_package(&self, pkg: &Package) -> Result<(), sqlx::Error> {
        debug!("db.add_package.adding", pkg.name(), pkg.version());
        sqlx::query(
            "INSERT OR REPLACE INTO packages (name, version, author, src, checksum, current) VALUES (?, ?, ?, ?, ?, 0)"
        )
        .bind(&pkg.name())
        .bind(&pkg.version().to_string())
        .bind(&pkg.author())
        .bind(&pkg.src().as_str())
        .bind(&pkg.checksum())
        .execute(&self.pool)
        .await?;
        debug!("db.add_package.added", pkg.name());
        Ok(())
    }

    /// Adds a package with its dependencies and installed files.
    pub async fn add_package_full(
        &self,
        pkg: &Package,
        installed_files: &[String],
    ) -> Result<(), sqlx::Error> {
        info!(
            "db.add_package_full.adding",
            pkg.name(),
            pkg.version(),
            installed_files.len()
        );

        self.add_package(pkg).await?;

        // Dependencies
        for (dep_name, dep_version) in pkg.dependencies() {
            debug!(
                "db.add_package_full.adding_dependency",
                &dep_name, &dep_version
            );
            sqlx::query(
                "INSERT OR REPLACE INTO dependencies (package_name, dependency_name, dependency_version) VALUES (?, ?, ?)"
            )
            .bind(&pkg.name())
            .bind(dep_name)
            .bind(&dep_version.to_string())
            .execute(&self.pool)
            .await?;
        }

        // Installed files
        for file_path in installed_files {
            debug!("db.add_package_full.adding_file", file_path);
            sqlx::query(
                "INSERT OR REPLACE INTO installed_files (package_name, file_path) VALUES (?, ?)",
            )
            .bind(&pkg.name())
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        }

        info!("db.add_package_full.success", pkg.name());
        Ok(())
    }

    /// Returns all files installed by a package.
    pub async fn get_installed_files(&self, pkg_name: &str) -> Result<Vec<String>, sqlx::Error> {
        debug!("db.get_installed_files.fetching", pkg_name);
        let rows = sqlx::query("SELECT file_path FROM installed_files WHERE package_name = ?")
            .bind(pkg_name)
            .fetch_all(&self.pool)
            .await?;

        let files: Vec<String> = rows
            .into_iter()
            .map(|row| row.get::<String, _>("file_path"))
            .collect();
        debug!("db.get_installed_files.found", files.len(), pkg_name);
        Ok(files)
    }

    /// Removes a package and its associated data from the database.
    pub async fn remove_package(&self, pkg_name: &str) -> Result<(), sqlx::Error> {
        info!("db.remove_package.removing", pkg_name);
        sqlx::query("DELETE FROM installed_files WHERE package_name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM dependencies WHERE package_name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM packages WHERE name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;
        info!("db.remove_package.removed", pkg_name);
        Ok(())
    }

    /// Returns the current version of a package, if installed.
    pub async fn get_package_version(&self, pkg_name: &str) -> Result<Option<String>, sqlx::Error> {
        debug!("db.get_package_version.fetching", pkg_name);
        let row = sqlx::query("SELECT version FROM packages WHERE name = ? AND current = 1")
            .bind(pkg_name)
            .fetch_optional(&self.pool)
            .await?;
        let result = row.map(|r| r.get::<String, _>("version"));
        debug!("db.get_package_version.result", pkg_name, &result);
        Ok(result)
    }

    /// Lists all installed packages.
    pub async fn list_packages(&self) -> Result<Vec<(String, String, bool)>, sqlx::Error> {
        debug!("db.list_packages.listing");
        let rows = sqlx::query("SELECT name, version, current FROM packages")
            .fetch_all(&self.pool)
            .await?;

        let mut packages = Vec::new();
        for row in rows {
            let name: String = row.get("name");
            let version: String = row.get("version");
            let current: bool = row.get("current");
            debug!("db.list_packages.found", &name, &version, current);
            packages.push((name, version, current));
        }

        Ok(packages)
    }

    /// Checks if a package is installed and returns its latest version.
    pub async fn is_installed(&self, name: &str) -> Result<Option<Version>, sqlx::Error> {
        debug!("db.is_installed.checking", name);
        let row = sqlx::query(
            "SELECT version FROM packages WHERE name = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            let ver_str: String = r.get("version");
            let ver = Version::parse(&ver_str).unwrap_or_else(|_| Version::new(0, 0, 0));
            debug!("db.is_installed.latest_version", name, &ver);
            Ok(Some(ver))
        } else {
            debug!("db.is_installed.not_found", name);
            Ok(None)
        }
    }

    /// Retrieves the current package metadata, including dependencies.
    pub async fn get_current_package(
        &self,
        pkg_name: &str,
    ) -> Result<Option<Package>, sqlx::Error> {
        debug!("db.get_current_package.fetching", pkg_name);
        let row = sqlx::query(
            "SELECT name, version, author, src, checksum FROM packages WHERE name = ? LIMIT 1",
        )
        .bind(pkg_name)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!("db.get_current_package.not_found", pkg_name);
                return Ok(None);
            }
        };

        // Dependencies
        let dep_rows = sqlx::query(
            "SELECT dependency_name, dependency_version FROM dependencies WHERE package_name = ?",
        )
        .bind(pkg_name)
        .fetch_all(&self.pool)
        .await?;

        let mut dependencies = Vec::new();
        for dep in dep_rows {
            let dep_name: String = dep.get("dependency_name");
            let dep_version_str: String = dep.get("dependency_version");
            if let Ok(dep_version) = Version::parse(&dep_version_str) {
                dependencies.push((dep_name, dep_version));
            }
        }

        let package = Package::new(
            row.get::<String, _>("name"),
            Version::parse(&row.get::<String, _>("version"))
                .unwrap_or_else(|_| Version::new(0, 0, 0)),
            row.get::<String, _>("author"),
            Source::Raw(row.get::<String, _>("src")),
            row.get::<String, _>("checksum"),
            dependencies,
        );

        debug!("db.get_current_package.retrieved", &package);
        Ok(Some(package))
    }

    /// Sets a specific version of a package as the current version.
    pub async fn set_current_version(
        &self,
        pkg_name: &str,
        version: &str,
    ) -> Result<(), sqlx::Error> {
        info!("db.set_current_version.setting", version, pkg_name);
        sqlx::query("UPDATE packages SET current = 0 WHERE name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE packages SET current = 1 WHERE name = ? AND version = ?")
            .bind(pkg_name)
            .bind(version)
            .execute(&self.pool)
            .await?;

        info!("db.set_current_version.success", version, pkg_name);
        Ok(())
    }

    /// Retrieves a specific version of a package by name and version string.
    pub async fn get_package_by_version(
        &self,
        pkg_name: &str,
        pkg_version: &str,
    ) -> Result<Option<Package>, sqlx::Error> {
        debug!("db.get_package_by_version.fetching", pkg_name, pkg_version);
        let row = sqlx::query(
            "SELECT name, version, author, src, checksum
             FROM packages
             WHERE name = ? AND version = ? LIMIT 1",
        )
        .bind(pkg_name)
        .bind(pkg_version)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!("db.get_package_by_version.not_found", pkg_name, pkg_version);
                return Ok(None);
            }
        };

        // Dependencies
        let dep_rows = sqlx::query(
            "SELECT dependency_name, dependency_version
             FROM dependencies
             WHERE package_name = ?",
        )
        .bind(pkg_name)
        .fetch_all(&self.pool)
        .await?;

        let mut dependencies = Vec::new();
        for dep in dep_rows {
            let dep_name: String = dep.get("dependency_name");
            let dep_version_str: String = dep.get("dependency_version");
            if let Ok(dep_version) = Version::parse(&dep_version_str) {
                dependencies.push((dep_name, dep_version));
            }
        }

        let package = Package::new(
            row.get::<String, _>("name"),
            Version::parse(&row.get::<String, _>("version"))
                .unwrap_or_else(|_| Version::new(0, 0, 0)),
            row.get::<String, _>("author"),
            Source::Raw(row.get::<String, _>("src")),
            row.get::<String, _>("checksum"),
            dependencies,
        );

        debug!("db.get_package_by_version.retrieved", &package);
        Ok(Some(package))
    }
}
