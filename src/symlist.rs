//! Symbolic link list management

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Symlist errors
#[derive(Debug, Error)]
pub enum SymlistError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Symlist parse error: {0}")]
    Parse(String),
}

/// Symlink entry
#[derive(Debug)]
pub struct SymlinkEntry {
    pub source: String,
    pub target: String,
}

/// Expands variables in paths
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

/// Parses symlist line
fn parse_symlist_line(line: &str) -> Result<SymlinkEntry, SymlistError> {
    let line = line.trim();

    if line.is_empty() || line.starts_with('#') {
        return Err(SymlistError::Parse("Empty or comment line".to_string()));
    }

    let parts: Vec<&str> = line.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return Err(SymlistError::Parse(format!(
            "Invalid line format, expected 'source target', got: {}",
            line
        )));
    }

    let source = parts[0].trim().to_string();
    let target = parts[1].trim().to_string();

    if source.is_empty() || target.is_empty() {
        return Err(SymlistError::Parse(
            "Source or target cannot be empty".to_string(),
        ));
    }

    Ok(SymlinkEntry { source, target })
}

/// Saves symlist template
pub fn save_template(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let symlist_template = r#"# Symlink list for package
# Format: <source_path> <target_path_with_variables>
#
# Available variables:
#   $HOME - user home directory
#   $XDG_DATA_HOME - user data directory (~/.local/share)
#   $XDG_CONFIG_HOME - user config directory (~/.config)
#   $XDG_BIN_HOME - user bin directory (~/.local/bin)

bin/my_binary $HOME/.local/bin/my_binary
share/applications/my_app.desktop $XDG_DATA_HOME/applications/my_app.desktop
"#;
    fs::write(path, symlist_template)?;
    Ok(())
}

/// Loads symlink list from file
pub fn load_symlist(
    path: &Path,
    package_root: &Path,
) -> Result<Vec<(PathBuf, PathBuf)>, SymlistError> {
    let content = fs::read_to_string(path)?;

    let mut entries = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        match parse_symlist_line(line) {
            Ok(entry) => entries.push(entry),
            Err(SymlistError::Parse(msg)) if msg.contains("Empty or comment") => continue,
            Err(e) => return Err(SymlistError::Parse(format!("Line {}: {}", line_num + 1, e))),
        }
    }

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
    fn test_parse_symlist_line() {
        let line = "/package/bin/foo $HOME/.local/bin/foo";
        let entry = parse_symlist_line(line).unwrap();
        assert_eq!(entry.source, "/package/bin/foo");
        assert_eq!(entry.target, "$HOME/.local/bin/foo");

        let line = "/package/bin/foo    $HOME/.local/bin/foo";
        let entry = parse_symlist_line(line).unwrap();
        assert_eq!(entry.source, "/package/bin/foo");
        assert_eq!(entry.target, "$HOME/.local/bin/foo");
    }

    #[test]
    fn test_parse_symlist_line_invalid() {
        let line = "/package/bin/foo";
        assert!(parse_symlist_line(line).is_err());

        let line = "";
        assert!(parse_symlist_line(line).is_err());

        let line = "# This is a comment";
        assert!(parse_symlist_line(line).is_err());
    }

    #[test]
    fn test_load_symlist_parsing() {
        let tmp_dir = tempdir().unwrap();
        let symlist_path = tmp_dir.path().join("symlist");

        let content = r#"# This is a comment line

bin/foo $HOME/.local/bin/foo
config/bar $XDG_CONFIG_HOME/bar

# Another comment
share/data $XDG_DATA_HOME/app_data
"#;
        fs::write(&symlist_path, content).unwrap();

        let package_root = tmp_dir.path();
        let symlinks = load_symlist(&symlist_path, package_root).unwrap();

        assert_eq!(symlinks.len(), 3);

        assert_eq!(symlinks[0].0, package_root.join("bin/foo"));
        assert_eq!(symlinks[1].0, package_root.join("config/bar"));
        assert_eq!(symlinks[2].0, package_root.join("share/data"));

        assert!(symlinks[0].1.to_string_lossy().ends_with(".local/bin/foo"));
        assert!(symlinks[1].1.to_string_lossy().ends_with("bar"));
        assert!(symlinks[2].1.to_string_lossy().ends_with("app_data"));
    }
}
