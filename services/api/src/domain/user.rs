//! Account preferences. Stored as JSON on the user; unknown or missing keys fall back to defaults, so
//! adding a preference never needs a migration.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    Light,
    Dark,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum DateFormat {
    #[default]
    #[serde(rename = "DD/MM/YYYY")]
    DayMonthYear,
    #[serde(rename = "YYYY-MM-DD")]
    Iso,
    #[serde(rename = "MM/DD/YYYY")]
    MonthDayYear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct NotificationPreferences {
    pub product_updates: bool,
    pub overdue_invoice_reminders: bool,
}

impl Default for NotificationPreferences {
    fn default() -> Self {
        Self {
            product_updates: false,
            overdue_invoice_reminders: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct Preferences {
    pub theme: ThemePreference,
    /// BCP 47 language tag for the interface.
    pub language: String,
    pub date_format: DateFormat,
    /// Locale used for number and currency formatting.
    pub number_locale: String,
    pub notifications: NotificationPreferences,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: ThemePreference::Auto,
            language: "en-NZ".into(),
            date_format: DateFormat::DayMonthYear,
            number_locale: "en-NZ".into(),
            notifications: NotificationPreferences::default(),
        }
    }
}

/// Interface languages available today.
pub const SUPPORTED_LANGUAGES: &[&str] = &["en-NZ"];
/// Number formatting locales offered in preferences.
pub const SUPPORTED_NUMBER_LOCALES: &[&str] = &["en-NZ", "en-AU", "en-GB", "en-US"];

impl Preferences {
    /// Reads stored JSON leniently: anything unreadable becomes the default.
    pub fn from_json(value: &serde_json::Value) -> Self {
        serde_json::from_value(value.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_keys_use_defaults() {
        let prefs = Preferences::from_json(&serde_json::json!({"theme": "dark"}));
        assert_eq!(prefs.theme, ThemePreference::Dark);
        assert_eq!(prefs.date_format, DateFormat::DayMonthYear);
        assert_eq!(prefs.language, "en-NZ");
    }

    #[test]
    fn date_format_serialises_as_pattern() {
        let json = serde_json::to_value(Preferences::default()).expect("serialises");
        assert_eq!(json["date_format"], "DD/MM/YYYY");
    }

    #[test]
    fn garbage_is_ignored() {
        assert_eq!(
            Preferences::from_json(&serde_json::json!("nope")),
            Preferences::default()
        );
    }
}
