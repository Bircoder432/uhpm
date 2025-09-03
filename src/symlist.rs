use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use thiserror::Error;
use serde::Deserialize;


#[derive(Debug, Error)]
pub enum SymlistError {
    #[error("Ошибка ввода-вывода: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ошибка парсинга RON: {0}")]
    Ron(#[from] ron::error::SpannedError),
}


#[derive(Debug, Deserialize)]
pub struct SymlinkEntry {
    pub source: String,
    pub target: String,
}


fn expand_vars(path: &str) -> PathBuf {
    let mut vars = HashMap::new();

    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();

        vars.insert("HOME".to_string(), home_str.clone());

        vars.insert(
            "XDG_DATA_HOME".to_string(),
            std::env::var("XDG_DATA_HOME")
                .unwrap_or_else(|_| format!("{}/.local/share", home_str)),
        );
        vars.insert(
            "XDG_CONFIG_HOME".to_string(),
            std::env::var("XDG_CONFIG_HOME")
                .unwrap_or_else(|_| format!("{}/.config", home_str)),
        );
        vars.insert(
            "XDG_BIN_HOME".to_string(),
            std::env::var("XDG_BIN_HOME")
                .unwrap_or_else(|_| format!("{}/.local/bin", home_str)),
        );
    }

    let mut expanded = path.to_string();
    for (key, value) in vars {
        expanded = expanded.replace(&format!("${}", key), &value);
    }

    PathBuf::from(expanded)
}


pub fn load_symlist(path: &Path) -> Result<Vec<(PathBuf, PathBuf)>, SymlistError> {
    let content = fs::read_to_string(path)?;
    let entries: Vec<SymlinkEntry> = ron::from_str(&content)?;

    Ok(entries
        .into_iter()
        .map(|e| {
            (
                PathBuf::from(e.source),
                expand_vars(&e.target),
            )
        })
        .collect())
}
