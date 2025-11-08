//! Package database management

use crate::package::{Package, Source};
use crate::{debug, info};
use semver::Version;
use sqlx::Row;
use sqlx::SqlitePool;
use std::fs;
use std::path::{Path, PathBuf};

/// Package database handler
pub struct PackageDB {
    pool: SqlitePool,
    path: PathBuf,
}

impl PackageDB {
    /// Creates new package database
    pub fn new(path: &Path) -> Result<Self, std::io::Error> {
        debug!("db.new.creating", path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            std::fs::File::create(path)?;
            debug!("db.new.file_created", path);
        }

        Ok(PackageDB {
            pool: SqlitePool::connect_lazy("sqlite::memory:")
                .expect("lazy pool must work for placeholder"),
            path: path.to_path_buf(),
        })
    }

    /// Initializes database tables
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
                package_version TEXT NOT NULL,
                file_path TEXT NOT NULL,
                PRIMARY KEY(package_name, package_version, file_path)
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

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Adds package to database
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

    /// Adds package with dependencies and installed files
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

        for file_path in installed_files {
            debug!("db.add_package_full.adding_file", file_path);
            sqlx::query(
                "INSERT OR REPLACE INTO installed_files (package_name, package_version, file_path) VALUES (?, ?, ?)",
            )
            .bind(&pkg.name())
            .bind(&pkg.version().to_string())
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        }

        info!("db.add_package_full.success", pkg.name());
        Ok(())
    }

    /// Gets installed files for package version
    pub async fn get_installed_files(
        &self,
        pkg_name: &str,
        pkg_version: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        debug!("db.get_installed_files.fetching", pkg_name, pkg_version);
        let rows = sqlx::query(
            "SELECT file_path FROM installed_files WHERE package_name = ? AND package_version = ?",
        )
        .bind(pkg_name)
        .bind(pkg_version)
        .fetch_all(&self.pool)
        .await?;

        let files: Vec<String> = rows
            .into_iter()
            .map(|row| row.get::<String, _>("file_path"))
            .collect();
        debug!(
            "db.get_installed_files.found",
            files.len(),
            pkg_name,
            pkg_version
        );
        Ok(files)
    }

    /// Gets all installed files for package
    pub async fn get_all_installed_files(
        &self,
        pkg_name: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        debug!("db.get_all_installed_files.fetching", pkg_name);
        let rows = sqlx::query("SELECT file_path FROM installed_files WHERE package_name = ?")
            .bind(pkg_name)
            .fetch_all(&self.pool)
            .await?;

        let files: Vec<String> = rows
            .into_iter()
            .map(|row| row.get::<String, _>("file_path"))
            .collect();
        debug!("db.get_all_installed_files.found", files.len(), pkg_name);
        Ok(files)
    }

    /// Removes specific package version
    pub async fn remove_package_version(
        &self,
        pkg_name: &str,
        pkg_version: &str,
    ) -> Result<(), sqlx::Error> {
        info!("db.remove_package_version.removing", pkg_name, pkg_version);
        sqlx::query("DELETE FROM installed_files WHERE package_name = ? AND package_version = ?")
            .bind(pkg_name)
            .bind(pkg_version)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM dependencies WHERE package_name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM packages WHERE name = ? AND version = ?")
            .bind(pkg_name)
            .bind(pkg_version)
            .execute(&self.pool)
            .await?;
        info!("db.remove_package_version.removed", pkg_name, pkg_version);
        Ok(())
    }

    /// Removes all package versions
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

    /// Gets current package version
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

    /// Gets latest package version
    pub async fn get_latest_package_version(
        &self,
        pkg_name: &str,
    ) -> Result<Option<Package>, sqlx::Error> {
        debug!("db.get_latest_package_version.fetching", pkg_name);

        let rows =
            sqlx::query("SELECT name, version, author, src, checksum FROM packages WHERE name = ?")
                .bind(pkg_name)
                .fetch_all(&self.pool)
                .await?;

        if rows.is_empty() {
            debug!("db.get_latest_package_version.not_found", pkg_name);
            return Ok(None);
        }

        let mut packages = Vec::new();
        for row in rows {
            let version_str: String = row.get("version");
            if let Ok(version) = Version::parse(&version_str) {
                packages.push((row, version));
            } else {
                debug!(
                    "db.get_latest_package_version.invalid_version",
                    pkg_name, &version_str
                );
            }
        }

        if packages.is_empty() {
            debug!("db.get_latest_package_version.no_valid_versions", pkg_name);
            return Ok(None);
        }

        let (latest_row, latest_version) = packages
            .into_iter()
            .max_by(|(_, a), (_, b)| a.cmp(b))
            .expect("packages is not empty");

        debug!(
            "db.get_latest_package_version.found",
            pkg_name, &latest_version
        );

        let package = Package::new(
            latest_row.get::<String, _>("name"),
            latest_version,
            latest_row.get::<String, _>("author"),
            Source::Raw(latest_row.get::<String, _>("src")),
            latest_row.get::<String, _>("checksum"),
            Vec::new(),
        );

        debug!("db.get_latest_package_version.retrieved", &package);
        Ok(Some(package))
    }

    /// Lists all installed packages
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

    /// Checks if package is installed
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

    /// Gets current package with dependencies
    pub async fn get_current_package(
        &self,
        pkg_name: &str,
    ) -> Result<Option<Package>, sqlx::Error> {
        debug!("db.get_current_package.fetching", pkg_name);
        let row = sqlx::query(
            "SELECT name, version, author, src, checksum FROM packages WHERE name = ? AND current = 1 LIMIT 1",
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

    /// Sets package version as current
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

    /// Gets package by specific version
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
