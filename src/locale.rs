//! locale.rs
//!
//! Provides localization support for UHPM.
//! Features:
//! - Automatic detection of system locale.
//! - Loading translations from `locale/<lang>.yml`.
//! - Retrieving localized messages.

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
    /// Initializes a Locale instance:
    /// - Detects system locale
    /// - Loads the corresponding translation file from `locale/<lang>.yml`
    pub fn initialize() -> Self {
        // Берём первые два символа локали: "en" вместо "en-US"
        let lang_full = get_locale().unwrap_or_else(|| "en".to_string());
        let lang = lang_full.chars().take(2).collect::<String>();

        let messages = Self::load_messages(&lang).unwrap_or_else(|err| {
            warn!("Failed to load locale '{}': {}", lang, err);
            HashMap::new()
        });

        Self { lang, messages }
    }

    /// Loads messages from YAML file
    fn load_messages(lang: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let path = Path::new("locale").join(format!("{}.yml", lang));
        if !path.exists() {
            return Err(format!("Locale file not found: {:?}", path).into());
        }

        let content = fs::read_to_string(&path)?;
        // Просто парсим весь YAML в HashMap<String, String>
        let map: HashMap<String, String> = serde_yaml::from_str(&content)?;
        Ok(map)
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

        let msg = locale.msg("welcome_message");
        println!("Localized message: {}", msg);

        assert!(locale.lang.len() == 2);
    }
}
