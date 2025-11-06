//! # Repository Module
//!
//! This module defines [`RepoDB`] and related utilities for managing package
//! repositories in **UHPM (Universal Home Package Manager)**.

use crate::error::RepoError;
use crate::fetcher;
use dirs;
use reqwest::Url;
use ron::from_str;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::env::home_dir;
use std::fs::{self, File};
use std::io::copy;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// SQLite-backed package repository database.
pub struct RepoDB {
    pool: SqlitePool,
}
pub type RepoMap = HashMap<String, String>;
#[derive(Serialize, Deserialize, Clone)]
pub enum RepoTypes {
    Binary,
    Source,
    Other,
}

#[derive(Serialize, Deserialize)]
pub struct RepoInfo {
    pub name: String,
    pub version: String,
    pub type_: RepoTypes,
}

impl RepoInfo {
    pub fn new(name: String, version: String, type_: RepoTypes) -> Self {
        RepoInfo {
            name,
            version,
            type_,
        }
    }

    pub fn parse_from_ron(ron: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(ron)
    }
}

impl RepoDB {
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Opens repository database from our repository structure
    pub async fn from_repo_path(repo_path: &Path) -> Result<Self, sqlx::Error> {
        let db_path = repo_path.join("repository.db");
        if !db_path.exists() {
            return Err(sqlx::Error::Configuration(
                "Repository database not found".into(),
            ));
        }

        let db_url = format!("sqlite://{}", db_path.to_str().unwrap());
        let pool = SqlitePool::connect(&db_url).await?;
        Ok(RepoDB { pool })
    }

    /// Opens (or creates) a new repository database at the given path
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

    /// Initializes tables for our repository structure
    async fn init_tables(&self) -> Result<(), sqlx::Error> {
        // Таблица пакетов (как в нашем uhprepo)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                packagename TEXT NOT NULL,
                pkgver TEXT NOT NULL,
                url TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Таблица исходников (как в нашем uhprepo)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                packagename TEXT NOT NULL,
                pkgver TEXT NOT NULL,
                url TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Индексы для быстрого поиска
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(packagename)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sources_name ON sources(packagename)")
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Получить URL пакета по имени и версии
    pub async fn get_package_url(&self, name: &str, version: &str) -> Result<String, RepoError> {
        let row = sqlx::query("SELECT url FROM packages WHERE packagename = ? AND pkgver = ?")
            .bind(name)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(r.get::<String, _>("url")),
            None => Err(RepoError::NotFound(format!("{}-{}", name, version))),
        }
    }

    /// Получить URL исходников пакета
    pub async fn get_source_url(&self, name: &str, version: &str) -> Result<String, RepoError> {
        let row = sqlx::query("SELECT url FROM sources WHERE packagename = ? AND pkgver = ?")
            .bind(name)
            .bind(version)
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(r) => Ok(r.get::<String, _>("url")),
            None => Err(RepoError::NotFound(format!("{}-{} sources", name, version))),
        }
    }

    /// Список всех пакетов в репозитории
    pub async fn list_packages(&self) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT packagename, pkgver, url FROM packages")
            .fetch_all(&self.pool)
            .await?;

        let packages = rows
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("packagename"),
                    r.get::<String, _>("pkgver"),
                    r.get::<String, _>("url"),
                )
            })
            .collect();

        Ok(packages)
    }

    /// Список всех исходников в репозитории
    pub async fn list_sources(&self) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT packagename, pkgver, url FROM sources")
            .fetch_all(&self.pool)
            .await?;

        let sources = rows
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("packagename"),
                    r.get::<String, _>("pkgver"),
                    r.get::<String, _>("url"),
                )
            })
            .collect();

        Ok(sources)
    }

    /// Поиск пакетов по имени
    pub async fn search_packages(
        &self,
        query: &str,
    ) -> Result<Vec<(String, String, String)>, sqlx::Error> {
        let search_pattern = format!("%{}%", query);
        let rows =
            sqlx::query("SELECT packagename, pkgver, url FROM packages WHERE packagename LIKE ?")
                .bind(&search_pattern)
                .fetch_all(&self.pool)
                .await?;

        let packages = rows
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("packagename"),
                    r.get::<String, _>("pkgver"),
                    r.get::<String, _>("url"),
                )
            })
            .collect();

        Ok(packages)
    }

    /// Добавить пакет в репозиторий (совместимо с нашим uhprepo)
    pub async fn add_package(
        &self,
        packagename: &str,
        pkgver: &str,
        url: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO packages (packagename, pkgver, url) VALUES (?, ?, ?)")
            .bind(packagename)
            .bind(pkgver)
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Добавить исходники в репозиторий (совместимо с нашим uhprepo)
    pub async fn add_source(
        &self,
        packagename: &str,
        pkgver: &str,
        url: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT OR REPLACE INTO sources (packagename, pkgver, url) VALUES (?, ?, ?)")
            .bind(packagename)
            .bind(pkgver)
            .bind(url)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Парсит конфигурацию репозиториев из RON файла
pub fn parse_repos<P: AsRef<Path>>(path: P) -> Result<RepoMap, RepoError> {
    let content = fs::read_to_string(path)?;
    let repos: HashMap<String, String> = from_str(&content).unwrap();
    Ok(repos)
}

pub async fn cache_repo(repos: RepoMap) -> Vec<PathBuf> {
    let mut repo_dbs: Vec<PathBuf> = Vec::new();
    for (name, url) in repos {
        let pathstr = format!(
            "{}/.uhpm/cache/repo/{}/repository.db",
            home_dir().unwrap().to_str().unwrap(),
            name,
        );
        let pathdb: PathBuf = PathBuf::from(pathstr);
        fetcher::download_file_to_path_with_dirs(&format!("{}/repository.db", url), &pathdb).await;
        repo_dbs.push(pathdb);
    }
    return repo_dbs;
}

/// Информация о репозитории из нашего info.json
#[derive(Serialize, Deserialize)]
pub struct RepositoryInfo {
    pub name: String,
    pub arch: String,
    pub description: String,
    pub package_count: usize,
    pub source_count: usize,
}

impl RepositoryInfo {
    pub fn load_from_path(repo_path: &Path) -> Result<Self, RepoError> {
        let info_path = repo_path.join("info.json");
        let content = fs::read_to_string(info_path)?;
        let info: RepositoryInfo = serde_json::from_str(&content).unwrap();
        Ok(info)
    }
}
