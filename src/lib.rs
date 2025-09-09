pub mod db;
pub mod fetcher;
pub mod package;
pub mod repo;
pub mod cli;
pub mod symlist;
pub mod self_remove;

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
