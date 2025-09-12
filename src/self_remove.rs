//! # Self-remove
//!
//! This module provides functionality for **self-removal** of the UHPM binary
//! and its associated configuration directory (`~/.uhpm`).
//!
//! ## How it works
//! 1. Determines the current binary path using [`std::env::current_exe`].
//! 2. Generates a temporary shell script (`uhpm_uninstall.sh`) in the user’s home directory.
//! 3. The script:
//!    - Waits briefly (`sleep 1`) to allow the UHPM process to exit.
//!    - Removes the UHPM binary itself.
//!    - Deletes the `~/.uhpm` configuration directory.
//!    - Prints a confirmation message.
//!    - Removes itself.
//! 4. Executes the script asynchronously via `bash`.
//!
//! ## Notes
//! - Only works on **Unix-like systems** (uses `PermissionsExt` and shell).
//! - On success, UHPM will remove itself and its data, leaving no trace.

use dirs;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

/// Removes UHPM from the system by deleting:
/// - The current executable binary
/// - The `~/.uhpm` directory
///
/// This is achieved by writing a temporary shell script (`uhpm_uninstall.sh`)
/// in the user’s home directory, marking it as executable, and running it.
/// The script performs cleanup and then deletes itself.
///
/// # Errors
/// - Returns an [`std::io::Error`] if file operations fail.
/// - Returns a generic error if the `$HOME` directory cannot be determined.
///
/// # Example
/// ```no_run
/// use uhpm::self_remove::self_remove;
///
/// fn main() {
///     if let Err(e) = self_remove() {
///         eprintln!("Failed to self-remove: {}", e);
///     }
/// }
/// ```
pub fn self_remove() -> Result<(), Box<dyn std::error::Error>> {
    // Path to current binary
    let exe_path: PathBuf = env::current_exe()?;
    // Path to ~/.uhpm directory
    let home_dir = dirs::home_dir().ok_or("Failed to get HOME directory")?;
    let uhpm_dir = home_dir.join(".uhpm");

    // Temporary uninstall script in $HOME
    let tmp_script = home_dir.join("uhpm_uninstall.sh");
    let script_content = format!(
        r#"#!/bin/bash
# Wait for the process to exit
sleep 1
rm -f "{}"
rm -rf "{}"
echo "UHPM has been removed"
rm -- "$0"
"#,
        exe_path.to_string_lossy(),
        uhpm_dir.to_string_lossy()
    );

    // Write and make script executable
    fs::write(&tmp_script, script_content)?;
    fs::set_permissions(&tmp_script, fs::Permissions::from_mode(0o755))?;

    // Execute uninstall script asynchronously
    Command::new("bash").arg(tmp_script).spawn()?;

    Ok(())
}
