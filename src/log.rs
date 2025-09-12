//! log.rs
//!
//! Localized logging with auto-detected system locale.
//! Provides macros: info!(), warn!(), debug!(), lprintln!(), lprint!().
//! Substitutes arguments `{}` in order, compatible with all Display types.

use crate::locale::Locale;
use once_cell::sync::Lazy;
use tracing::{debug as t_debug, info as t_info, warn as t_warn};

/// Global static logger
pub static LOGGER: Lazy<Locale> = Lazy::new(|| Locale::initialize());

/// Internal helper: replaces `{}` placeholders with provided arguments
pub fn format_ordered(template: &str, args: &[String]) -> String {
    let mut result = String::new();
    let mut parts = template.split("{}");
    let mut iter = args.iter();

    if let Some(first) = parts.next() {
        result.push_str(first);
    }

    for part in parts {
        if let Some(arg) = iter.next() {
            result.push_str(arg);
        }
        result.push_str(part);
    }

    result
}

/// Localized info macro
#[macro_export]
macro_rules! info {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::info!(target: "uhpm", "{}", msg);
        }
    };
}

/// Localized warn macro
#[macro_export]
macro_rules! warn {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::warn!(target: "uhpm", "{}", msg);
        }
    };
}

/// Localized debug macro
#[macro_export]
macro_rules! debug {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::debug!(target: "uhpm", "{}", msg);
        }
    };
}

/// Localized println macro
#[macro_export]
macro_rules! lprintln {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            println!("{}", msg);
        }
    };
}

/// Localized print macro
#[macro_export]
macro_rules! lprint {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$(format!("{}", $arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            print!("{}", msg);
        }
    };
}
