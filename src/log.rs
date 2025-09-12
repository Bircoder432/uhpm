//! log.rs
//!
//! Localized logging with auto-detected system locale.
//! Provides macros: info!(), warn!(), debug!(), error!(), lprintln!(), lprint!().
//! Supports multiple arguments of any type and substitutes them in order.

use crate::locale::Locale;
use once_cell::sync::Lazy;

/// Global static logger
pub static LOGGER: Lazy<Locale> = Lazy::new(|| Locale::initialize());

/// Helper: replaces `{}` placeholders in template with provided arguments
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

/// Internal helper to format any type with Debug
pub fn fmt_debug<T: std::fmt::Debug>(val: T) -> String {
    format!("{:?}", val)
}

// -------------------- MACROS -------------------- //

#[macro_export]
macro_rules! info {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::info!(target: "uhpm", "{}", msg);
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::warn!(target: "uhpm", "{}", msg);
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::debug!(target: "uhpm", "{}", msg);
        }
    };
}

#[macro_export]
macro_rules! error {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            tracing::error!(target: "uhpm", "{}", msg);
        }
    };
}

#[macro_export]
macro_rules! lprintln {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            println!("{}", msg);
        }
    };
}

#[macro_export]
macro_rules! lprint {
    ($key:expr $(, $arg:expr)*) => {
        {
            let template = $crate::log::LOGGER.msg($key);
            let args: Vec<String> = vec![$($crate::log::fmt_debug($arg)),*];
            let msg = $crate::log::format_ordered(&template, &args);
            print!("{}", msg);
        }
    };
}
