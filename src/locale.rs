//! locale.rs
//!
//! Provides localization support for UHPM.
//! Features:
//! - Automatic detection of system locale
//! - Loading translations from `locale/<lang>.ron`
//! - Retrieving localized messages

use std::{collections::HashMap, fs, path::Path};
use sys_locale::get_locale;
use tracing::warn;

/// Main struct for localization
#[derive(Debug)]
pub struct Locale {
    /// Active language code, e.g., "en", "ru"
    pub lang: String,
    /// Loaded localized messages
    pub messages: HashMap<String, String>,
}

impl Locale {
    /// Initializes a Locale instance
    /// - Detects system locale
    /// - Loads the corresponding translation file from `locale/<lang>.ron`
    pub fn initialize() -> Self {
        let lang_full = get_locale().unwrap_or_else(|| "en".to_string());
        let lang = lang_full.chars().take(2).collect::<String>();
        let messages = Self::load_messages(&lang).unwrap_or_else(|err| {
            warn!("Failed to load locale '{}': {}", lang, err);
            HashMap::new()
        });

        Self { lang, messages }
    }

    /// Loads messages from RON file and flattens the structure
    fn load_messages(lang: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        #[cfg(debug_assertions)]
        let path = Path::new("locale").join(format!("{}.ron", lang));

        #[cfg(not(debug_assertions))]
        let path = dirs::home_dir()
            .unwrap()
            .join(".uhpm")
            .join("locale")
            .join(format!("{}.ron", lang));

        if !path.exists() {
            return Err(format!("Locale file not found: {:?}", path).into());
        }

        let content = fs::read_to_string(&path)?;

        // Parse RON into Value
        let value: ron::Value = ron::from_str(&content)?;

        // Recursively collect all strings into a flat HashMap
        let mut messages = HashMap::new();
        Self::flatten_value(value, &mut messages, String::new());

        Ok(messages)
    }

    /// Recursively traverses RON structure and collects all strings
    fn flatten_value(
        value: ron::Value,
        messages: &mut HashMap<String, String>,
        current_key: String,
    ) {
        match value {
            ron::Value::Map(map) => {
                for (key, value) in map.into_iter() {
                    // Only string keys are processed
                    if let ron::Value::String(key_str) = key {
                        let new_key = if current_key.is_empty() {
                            key_str
                        } else {
                            format!("{}.{}", current_key, key_str)
                        };
                        Self::flatten_value(value, messages, new_key);
                    }
                    // Other key types are ignored
                }
            }
            ron::Value::String(s) => {
                messages.insert(current_key, s);
            }
            _ => {} // Ignore numbers, booleans, etc. - only strings matter
        }
    }

    /// Retrieves a localized message by key
    /// Falls back to the key itself if translation is missing
    pub fn msg(&self, key: &str) -> String {
        self.messages
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber;

    #[test]
    fn test_locale_load() {
        tracing_subscriber::fmt::init();

        let locale = Locale::initialize();
        println!("Active locale: {}", locale.lang);

        let msg = locale.msg("main.info.uhpm_started");
        println!("Localized message: {}", msg);

        assert!(locale.lang.len() == 2 || locale.lang.len() == 1);
    }
}
