use ron::from_str;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;


#[derive(Error, Debug)]
pub enum RepoError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("Package not found: {0}")]
    NotFound(String),
}

pub struct RepoDB {
    pool: SqlitePool,
}

impl RepoDB {
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

pub fn parse_repos<P: AsRef<Path>>(path: P) -> Result<HashMap<String, String>, RepoError> {
    let content = fs::read_to_string(path)?;
    let repos: HashMap<String, String> = from_str(&content).unwrap();
    Ok(repos)
}
