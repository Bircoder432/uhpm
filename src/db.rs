use crate::package::{Package, Source};
use semver::Version;
use sqlx::Row;
use sqlx::{Executor, Pool, Sqlite, SqlitePool};
use std::fs;
use std::path::Path;
use tracing::{info, debug, warn, error};

pub struct PackageDB {
    pool: SqlitePool,
}

impl PackageDB {
    pub async fn new(path: &Path) -> Result<Self, sqlx::Error> {
        debug!("Инициализация базы данных: {:?}", path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("Failed to create directory for database");
            }
            std::fs::File::create(path).expect("Cannot create database file");
            debug!("Создан новый файл базы данных: {:?}", path);
        }

        let path_str = path.to_str().expect("Invalid UTF-8 path");
        let db_url = format!("sqlite://{}", path_str);
        debug!("Подключение к базе: {}", db_url);

        let pool = SqlitePool::connect(&db_url).await?;
        let db = PackageDB { pool };
        db.init_tables().await?;
        info!("База данных инициализирована");

        Ok(db)
    }

    async fn init_tables(&self) -> Result<(), sqlx::Error> {
        debug!("Создание таблиц, если не существуют");
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

        debug!("Таблицы созданы / проверены");
        Ok(())
    }

    pub async fn add_package(&self, pkg: &Package) -> Result<(), sqlx::Error> {
        debug!("Добавляем пакет {} версии {}", pkg.name(), pkg.version());
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
        debug!("Пакет {} добавлен", pkg.name());
        Ok(())
    }

    pub async fn add_package_full(
        &self,
        pkg: &Package,
        installed_files: &[String],
    ) -> Result<(), sqlx::Error> {
        info!("Добавляем пакет {} версии {} с {} файлами", pkg.name(), pkg.version(), installed_files.len());

        self.add_package(pkg).await?;

        for (dep_name, dep_version) in pkg.dependencies() {
            debug!("Добавляем зависимость {} -> {}", dep_name, dep_version);
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
            debug!("Добавляем файл пакета в базу: {}", file_path);
            sqlx::query(
                "INSERT OR REPLACE INTO installed_files (package_name, file_path) VALUES (?, ?)",
            )
            .bind(&pkg.name())
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        }

        info!("Пакет {} успешно добавлен в базу", pkg.name());
        Ok(())
    }

    pub async fn get_installed_files(&self, pkg_name: &str) -> Result<Vec<String>, sqlx::Error> {
        debug!("Получаем установленные файлы для пакета {}", pkg_name);
        let rows = sqlx::query("SELECT file_path FROM installed_files WHERE package_name = ?")
            .bind(pkg_name)
            .fetch_all(&self.pool)
            .await?;

        let files: Vec<String> = rows.into_iter().map(|row| row.get::<String, _>("file_path")).collect();
        debug!("Найдено {} файлов для пакета {}", files.len(), pkg_name);
        Ok(files)
    }

    pub async fn remove_package(&self, pkg_name: &str) -> Result<(), sqlx::Error> {
        info!("Удаляем пакет {} из базы", pkg_name);
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
        info!("Пакет {} удалён", pkg_name);
        Ok(())
    }

    pub async fn get_package_version(&self, pkg_name: &str) -> Result<Option<String>, sqlx::Error> {
        debug!("Получаем текущую версию пакета {}", pkg_name);
        let row = sqlx::query("SELECT version FROM packages WHERE name = ? AND current = 1")
            .bind(pkg_name)
            .fetch_optional(&self.pool)
            .await?;
        let result = row.map(|r| r.get::<String, _>("version"));
        debug!("Текущая версия пакета {}: {:?}", pkg_name, result);
        Ok(result)
    }

    pub async fn list_packages(&self) -> Result<Vec<(String, String, bool)>, sqlx::Error> {
        debug!("Список всех пакетов в базе");
        let rows = sqlx::query("SELECT name, version, current FROM packages")
            .fetch_all(&self.pool)
            .await?;

        let mut packages = Vec::new();
        for row in rows {
            let name: String = row.get("name");
            let version: String = row.get("version");
            let current: bool = row.get("current");
            debug!("Найден пакет: {} {} (current={})", name, version, current);
            packages.push((name, version, current));
        }

        Ok(packages)
    }

    pub async fn is_installed(&self, name: &str) -> Result<Option<Version>, sqlx::Error> {
        debug!("Проверяем, установлена ли версия пакета {}", name);
        let row = sqlx::query(
            "SELECT version FROM packages WHERE name = ? ORDER BY version DESC LIMIT 1",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(r) = row {
            let ver_str: String = r.get("version");
            let ver = Version::parse(&ver_str).unwrap_or_else(|_| Version::new(0, 0, 0));
            debug!("Последняя версия пакета {}: {}", name, ver);
            Ok(Some(ver))
        } else {
            debug!("Пакет {} не найден в базе", name);
            Ok(None)
        }
    }

    pub async fn get_current_package(&self, pkg_name: &str) -> Result<Option<Package>, sqlx::Error> {
        debug!("Получаем текущий пакет {}", pkg_name);
        let row = sqlx::query(
            "SELECT name, version, author, src, checksum FROM packages WHERE name = ? LIMIT 1"
        )
        .bind(pkg_name)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!("Пакет {} не найден", pkg_name);
                return Ok(None);
            },
        };

        let dep_rows = sqlx::query(
            "SELECT dependency_name, dependency_version FROM dependencies WHERE package_name = ?"
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
            Version::parse(&row.get::<String, _>("version")).unwrap_or_else(|_| Version::new(0, 0, 0)),
            row.get::<String, _>("author"),
            Source::Raw(row.get::<String, _>("src")),
            row.get::<String, _>("checksum"),
            dependencies,
        );

        debug!("Получен пакет: {:?}", package);
        Ok(Some(package))
    }

    pub async fn set_current_version(
        &self,
        pkg_name: &str,
        version: &str,
    ) -> Result<(), sqlx::Error> {
        info!("Устанавливаем current версию {} для пакета {}", version, pkg_name);
        sqlx::query("UPDATE packages SET current = 0 WHERE name = ?")
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;

        sqlx::query("UPDATE packages SET current = 1 WHERE name = ? AND version = ?")
            .bind(pkg_name)
            .bind(version)
            .execute(&self.pool)
            .await?;

        info!("Версия {} установлена как current для пакета {}", version, pkg_name);
        Ok(())
    }

    pub async fn get_package_by_version(
        &self,
        pkg_name: &str,
        pkg_version: &str,
    ) -> Result<Option<Package>, sqlx::Error> {
        debug!("Получаем пакет {} версии {}", pkg_name, pkg_version);
        let row = sqlx::query(
            "SELECT name, version, author, src, checksum
             FROM packages
             WHERE name = ? AND version = ? LIMIT 1"
        )
        .bind(pkg_name)
        .bind(pkg_version)
        .fetch_optional(&self.pool)
        .await?;

        let row = match row {
            Some(r) => r,
            None => {
                debug!("Пакет {} версии {} не найден", pkg_name, pkg_version);
                return Ok(None);
            },
        };

        let dep_rows = sqlx::query(
            "SELECT dependency_name, dependency_version
             FROM dependencies
             WHERE package_name = ?"
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
            Version::parse(&row.get::<String, _>("version")).unwrap_or_else(|_| Version::new(0, 0, 0)),
            row.get::<String, _>("author"),
            Source::Raw(row.get::<String, _>("src")),
            row.get::<String, _>("checksum"),
            dependencies,
        );

        debug!("Получен пакет: {:?}", package);
        Ok(Some(package))
    }
}
