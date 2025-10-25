pub mod cli;
pub mod config;
pub mod db;
pub mod error;
pub mod fetcher;
pub mod locale;
pub mod log;
pub mod package;
pub mod repo;
pub mod service;
pub mod symlist;

use std::fs;

pub fn clear_tmp() -> std::io::Result<()> {
    let mut tmp_dir = dirs::home_dir().unwrap();
    tmp_dir.push(".uhpm/tmp");

    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
        fs::create_dir_all(&tmp_dir)?;
    }

    Ok(())
}
