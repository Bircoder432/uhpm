//! Localized logging macros

use crate::locale::Locale;
use once_cell::sync::Lazy;

/// Global locale instance
pub static LOGGER: Lazy<Locale> = Lazy::new(Locale::initialize);

/// Formats template with ordered arguments
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

/// Formats any debug value as string
pub fn fmt_debug<T: std::fmt::Debug>(val: T) -> String {
    format!("{:?}", val)
}

// Logging macros

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
