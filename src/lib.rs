//! # UHPM (Universal Home Package Manager)
//!
//! UHPM is a lightweight, user-space package manager designed for managing
//! packages in the user's home directory (`~/.uhpm`).
//!
//! ## Features
//! - Package installation from local files or remote repositories.
//! - Tracking installed files and dependencies.
//! - Version management with the ability to switch between versions.
//! - Package updates from repositories.
//! - Removal of packages and their associated files.
//! - Self-removal of UHPM itself.
//!
//! ## Modules
//! - [`cli`] — Command-line interface powered by `clap`.
//! - [`db`] — SQLite-based database for tracking packages.
//! - [`fetcher`] — Downloading and installing packages from URLs.
//! - [`package`] — Core package logic (installer, remover, switcher, updater).
//! - [`repo`] — Repository handling (package indexing).
//! - [`self_remove`] — Logic for uninstalling UHPM itself.
//! - [`symlist`] — Symbolic link management for installed files.
//!
//! ## Example
//! ```rust,no_run
//! use uhpm::clear_tmp;
//!
//! fn main() {
//!     // Clear UHPM temporary directory (~/.uhpm/tmp).
//!     clear_tmp().unwrap();
//! }
//! ```

pub mod cli;
pub mod db;
pub mod fetcher;
pub mod locale;
pub mod log;
pub mod package;
pub mod repo;
pub mod self_remove;
pub mod symlist;

use std::fs;

/// Clears the UHPM temporary directory (`~/.uhpm/tmp`).
///
/// This function removes the entire temporary directory and recreates it,
/// ensuring that any leftover files from previous operations are deleted.
///
/// # Errors
/// Returns [`std::io::Error`] if the directory cannot be removed or created.
pub fn clear_tmp() -> std::io::Result<()> {
    let mut tmp_dir = dirs::home_dir().unwrap();
    tmp_dir.push(".uhpm/tmp");

    if tmp_dir.exists() {
        fs::remove_dir_all(&tmp_dir)?;
        fs::create_dir_all(&tmp_dir)?;
    }

    Ok(())
}
