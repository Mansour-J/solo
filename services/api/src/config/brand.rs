//! Product identity. The defaults come from `branding/brand.json` (embedded at compile time); every
//! user-facing value can be overridden at runtime with an `APP_*` environment variable, so a deployment
//! can be renamed without code changes. Nothing else in the API hardcodes the product name.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const BRAND_JSON: &str = include_str!("../../../../branding/brand.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrandFile {
    name: String,
    short_name: String,
    description: String,
    colors: BrandColorsFile,
    email: BrandEmailFile,
    urls: BrandUrlsFile,
    copyright: String,
}

#[derive(Debug, Deserialize)]
struct BrandColorsFile {
    primary: String,
    secondary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrandEmailFile {
    sender_name: String,
    sender_address: String,
    support_email: String,
}

#[derive(Debug, Deserialize)]
struct BrandUrlsFile {
    website: String,
    app: String,
}

/// Public, user-facing identity. Served to clients by `GET /api/v1/meta`.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Brand {
    pub name: String,
    pub short_name: String,
    pub description: String,
    pub primary_color: String,
    pub secondary_color: String,
    pub support_email: String,
    pub website_url: String,
    /// Base URL of the web application; used to build links in emails.
    pub app_url: String,
    pub copyright: String,
    #[serde(skip)]
    pub email_sender_name: String,
    #[serde(skip)]
    pub email_sender_address: String,
}

impl Brand {
    /// Loads the embedded defaults and applies `APP_*` overrides from `lookup`.
    pub fn load(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let file: BrandFile = serde_json::from_str(BRAND_JSON)?;
        let pick = |key: &str, default: String| lookup(key).unwrap_or(default);
        let name = pick("APP_NAME", file.name);
        let brand = Self {
            short_name: pick("APP_SHORT_NAME", file.short_name),
            description: pick("APP_DESCRIPTION", file.description),
            primary_color: pick("APP_PRIMARY_COLOR", file.colors.primary),
            secondary_color: pick("APP_SECONDARY_COLOR", file.colors.secondary),
            support_email: pick("APP_SUPPORT_EMAIL", file.email.support_email),
            website_url: pick("APP_WEBSITE_URL", file.urls.website),
            app_url: pick("APP_URL", file.urls.app)
                .trim_end_matches('/')
                .to_owned(),
            copyright: pick("APP_COPYRIGHT", file.copyright),
            email_sender_name: pick("APP_EMAIL_SENDER_NAME", file.email.sender_name),
            email_sender_address: pick("APP_EMAIL_FROM", file.email.sender_address),
            name,
        };
        for (key, colour) in [
            ("APP_PRIMARY_COLOR", &brand.primary_color),
            ("APP_SECONDARY_COLOR", &brand.secondary_color),
        ] {
            if !is_hex_colour(colour) {
                anyhow::bail!("{key} must be a #rrggbb colour, got {colour:?}");
            }
        }
        url::Url::parse(&brand.app_url)
            .map_err(|e| anyhow::anyhow!("APP_URL is not a valid URL: {e}"))?;
        Ok(brand)
    }

    /// A link into the web app. The client uses hash routing, so `path` goes after `#`.
    pub fn app_link(&self, path: &str) -> String {
        format!("{}/#{}", self.app_url, path)
    }
}

fn is_hex_colour(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_defaults_load() {
        let brand = Brand::load(|_| None).expect("brand.json parses");
        assert!(!brand.name.is_empty());
        assert!(is_hex_colour(&brand.primary_color));
    }

    #[test]
    fn environment_overrides_win() {
        let brand = Brand::load(|key| match key {
            "APP_NAME" => Some("Renamed".into()),
            "APP_URL" => Some("https://books.example.com/".into()),
            _ => None,
        })
        .expect("overrides apply");
        assert_eq!(brand.name, "Renamed");
        assert_eq!(brand.app_url, "https://books.example.com");
        assert_eq!(
            brand.app_link("/verify-email?token=x"),
            "https://books.example.com/#/verify-email?token=x"
        );
    }

    #[test]
    fn invalid_colour_is_rejected() {
        let result = Brand::load(|key| (key == "APP_PRIMARY_COLOR").then(|| "green".into()));
        assert!(result.is_err());
    }
}
