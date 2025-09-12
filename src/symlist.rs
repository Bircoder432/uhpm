//! # Symlist
//!
//! This module handles **symbolic link lists** (`symlist.ron`),
//! which describe what files from an installed package should be symlinked
//! into user/system paths.
//!
//! ## Example format (`symlist.ron`)
//! ```ron
//! [
//!     (
//!         source: "bin/my_binary",                 // path inside the package
//!         target: "$HOME/.local/bin/my_binary",    // target path (with env vars)
//!     ),
//!     (
//!         source: "share/applications/app.desktop",
//!         target: "$XDG_DATA_HOME/applications/app.desktop",
//!     ),
//! ]
//! ```
//!
//! ## Supported variables
//! - `$HOME` — user home directory
//! - `$XDG_DATA_HOME` — user data directory (defaults to `~/.local/share`)
//! - `$XDG_CONFIG_HOME` — user config directory (defaults to `~/.config`)
//! - `$XDG_BIN_HOME` — user bin directory (defaults to `~/.local/bin`)
//!
//! These variables are automatically expanded in target paths.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Possible errors when working with symlists
#[derive(Debug, Error)]
pub enum SymlistError {
    /// Input/output error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// RON parsing error
    #[error("RON parse error: {0}")]
    Ron(#[from] ron::error::SpannedError),
}

/// Entry in the symlink list
#[derive(Debug, Deserialize)]
pub struct SymlinkEntry {
    /// Relative path inside the package
    pub source: String,
    /// Target path (with variables)
    pub target: String,
}

/// Expands variables (`$HOME`, `$XDG_*`) in paths
fn expand_vars(path: &str) -> PathBuf {
    let mut vars = HashMap::new();

    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();

        vars.insert("HOME".to_string(), home_str.clone());

        vars.insert(
            "XDG_DATA_HOME".to_string(),
            std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home_str)),
        );
        vars.insert(
            "XDG_CONFIG_HOME".to_string(),
            std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{}/.config", home_str)),
        );
        vars.insert(
            "XDG_BIN_HOME".to_string(),
            std::env::var("XDG_BIN_HOME").unwrap_or_else(|_| format!("{}/.local/bin", home_str)),
        );
    }

    let mut expanded = path.to_string();
    for (key, value) in vars {
        expanded = expanded.replace(&format!("${}", key), &value);
    }

    PathBuf::from(expanded)
}

/// Saves a symlist template (`symlist.ron`)
///
/// Useful for package initialization or project bootstrapping.
///
/// # Example
/// ```no_run
/// use uhpm::symlist::save_template;
/// use std::path::Path;
///
/// save_template(Path::new("symlist.ron")).unwrap();
/// ```
pub fn save_template(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let symlist_template = r#"[
    (
        source: "bin/my_binary",          // path inside the package
        target: "$HOME/.local/bin/my_binary", // target path
    ),
    (
        source: "share/applications/my_app.desktop",
        target: "$XDG_DATA_HOME/applications/my_app.desktop",
    ),
]
"#;
    fs::write(path, symlist_template)?;
    Ok(())
}

/// Loads a symlink list from `symlist.ron`
///
/// - `path` — path to the symlist file
/// - `package_root` — root directory of the package
///
/// Returns a vector of `(source_path, target_path)`.
///
/// # Errors
/// - [`SymlistError::Io`] — if the file cannot be read
/// - [`SymlistError::Ron`] — if the file has an invalid format
///
/// # Example
/// ```no_run
/// use uhpm::symlist::load_symlist;
/// use std::path::Path;
///
/// let symlinks = load_symlist(Path::new("symlist.ron"), Path::new("/tmp/pkg_root")).unwrap();
/// for (src, dst) in symlinks {
///     println!("{} -> {}", src.display(), dst.display());
/// }
/// ```
pub fn load_symlist(
    path: &Path,
    package_root: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, SymlistError> {
    let content = fs::read_to_string(path)?;
    let entries: Vec<SymlinkEntry> = ron::from_str(&content)?;

    Ok(entries
        .into_iter()
        .map(|e| {
            let src = package_root.join(e.source);
            let dst = expand_vars(&e.target);
            (src, dst)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_expand_vars_home() {
        let home = dirs::home_dir().unwrap();
        let path = "$HOME/test_folder";
        let expanded = expand_vars(path);
        assert_eq!(expanded, home.join("test_folder"));
    }

    #[test]
    fn test_expand_vars_xdg() {
        let home = dirs::home_dir().unwrap();
        let xdg_data = std::env::var("XDG_DATA_HOME")
            .unwrap_or_else(|_| format!("{}/.local/share", home.to_string_lossy()));
        let path = "$XDG_DATA_HOME/some_dir";
        let expanded = expand_vars(path);
        assert_eq!(expanded, PathBuf::from(xdg_data).join("some_dir"));
    }

    #[test]
    fn test_load_symlist_parsing() {
        let tmp_dir = tempdir().unwrap();
        let symlist_path = tmp_dir.path().join("symlist.ron");

        let content = r#"
        [
            (source: "bin/foo", target: "$HOME/.local/bin/foo"),
            (source: "config/bar", target: "$XDG_CONFIG_HOME/bar")
        ]
        "#;
        fs::write(&symlist_path, content).unwrap();

        let package_root = tmp_dir.path();
        let symlinks = load_symlist(&symlist_path, package_root).unwrap();

        assert_eq!(symlinks.len(), 2);

        assert_eq!(symlinks[0].0, package_root.join("bin/foo"));
        assert_eq!(symlinks[1].0, package_root.join("config/bar"));
        assert!(symlinks[0].1.to_string_lossy().ends_with(".local/bin/foo"));
        assert!(symlinks[1].1.to_string_lossy().ends_with("bar"));
    }
}
