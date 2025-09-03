
use sqlx::{SqlitePool, Pool, Sqlite, Executor};
use sqlx::Row;
use semver::Version;
use std::path::Path;
use std::fs;
use crate::package::Package;

pub struct PackageDB {
    pool: SqlitePool,
}

impl PackageDB {
    /// Создаём новый объект PackageDB, автоматически создавая базу если её нет
    pub async fn new(path: &Path) -> Result<Self, sqlx::Error> {
        // Проверяем существует ли база
        if !path.exists() {
            // Создаём директорию, если её нет
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("Failed to create directory for database");
            }
            std::fs::File::create(path).expect("Cannot create database file");
            // SQLite создаст файл при подключении автоматически
        }

        // Конвертируем Path в строку для sqlx
        let path_str = path.to_str().expect("Invalid UTF-8 path");
        let db_url = format!("sqlite://{}", path_str);

        // Подключаемся к базе
        let pool = SqlitePool::connect(&db_url).await?;

        let db = PackageDB { pool };

        // Инициализация таблиц (если их ещё нет)
        db.init_tables().await?;

        Ok(db)
    }

    /// Инициализация таблиц
    async fn init_tables(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                author TEXT NOT NULL,
                src TEXT NOT NULL,
                checksum TEXT NOT NULL
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
        // Тут можно добавить таблицу dependencies, если нужно

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

        Ok(())
    }

    // Примеры методов для работы с пакетами
    pub async fn add_package(&self, pkg: &Package) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO packages (name, version, author, src, checksum) VALUES (?, ?, ?, ?, ?)")
            .bind(&pkg.name())
            .bind(&pkg.version().to_string())
            .bind(&pkg.author())
            .bind(&pkg.src().as_str())
            .bind(&pkg.checksum())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn add_package_full(
        &self,
        pkg: &Package,
        installed_files: &[String]
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO packages (name, version, author, src, checksum) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(&pkg.name())
        .bind(&pkg.version().to_string())
        .bind(&pkg.author())
        .bind(&pkg.src().as_str())
        .bind(&pkg.checksum())
        .execute(&self.pool)
        .await?;

        for (dep_name, dep_version) in pkg.dependencies() {
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
            sqlx::query(
                "INSERT OR REPLACE INTO installed_files (package_name, file_path) VALUES (?, ?)"
            )
            .bind(&pkg.name())
            .bind(file_path)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }
    pub async fn get_installed_files(&self, pkg_name: &str) -> Result<Vec<String>, sqlx::Error> {
            let rows = sqlx::query(
                "SELECT file_path FROM installed_files WHERE package_name = ?"
            )
            .bind(pkg_name)
            .fetch_all(&self.pool)
            .await?;

            let files = rows.into_iter()
                .map(|row| row.get::<String, _>("file_path"))
                .collect();

            Ok(files)
        }

        pub async fn remove_package(&self, pkg_name: &str) -> Result<(), sqlx::Error> {
            sqlx::query(
                "DELETE FROM installed_files WHERE package_name = ?"
            )
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                "DELETE FROM dependencies WHERE package_name = ?"
            )
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;

            sqlx::query(
                "DELETE FROM packages WHERE name = ?"
            )
            .bind(pkg_name)
            .execute(&self.pool)
            .await?;

            Ok(())
        }
        pub async fn get_package_version(&self, pkg_name: &str) -> Result<Option<String>, sqlx::Error> {
            let row = sqlx::query("SELECT version FROM packages WHERE name = ?")
                .bind(pkg_name)
                .fetch_optional(&self.pool)
                .await?;
            Ok(row.map(|r| r.get::<String, _>("version")))
        }

        pub async fn list_packages(&self) -> Result<Vec<(String, String)>, sqlx::Error> {
                let rows = sqlx::query("SELECT name, version FROM packages")
                    .fetch_all(&self.pool)
                    .await?;

                let mut packages = Vec::new();
                for row in rows {
                    let name: String = row.get("name");
                    let version: String = row.get("version");
                    packages.push((name, version));
                }

                Ok(packages)
            }



            pub async fn is_installed(&self, name: &str) -> Result<Option<Version>, sqlx::Error> {
                let row = sqlx::query("SELECT version FROM packages WHERE name = ? ORDER BY version DESC LIMIT 1")
                    .bind(name)
                    .fetch_optional(&self.pool)
                    .await?;

                if let Some(r) = row {
                    let ver_str: String = r.get("version");
                    let ver = Version::parse(&ver_str).unwrap_or_else(|_| Version::new(0,0,0));
                    Ok(Some(ver))
                } else {
                    Ok(None)
                }
            }


    }
