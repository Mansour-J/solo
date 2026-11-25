//! Runtime configuration, read once from environment variables at start-up.
//! Every variable is documented in `docs/deployment/environment-variables.md`.

mod brand;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, bail};

pub use brand::Brand;

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub bind_addr: SocketAddr,
    pub database: DatabaseConfig,
    pub email: EmailConfig,
    pub storage: StorageConfig,
    pub http: HttpConfig,
    pub brand: Brand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Test,
    Production,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum EmailConfig {
    Smtp(SmtpConfig),
    /// Logs emails instead of sending them (development without a mail server).
    Log,
    /// Keeps emails in memory (tests).
    Memory,
}

#[derive(Debug, Clone)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub tls: SmtpTls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtpTls {
    None,
    StartTls,
    Tls,
}

#[derive(Debug, Clone)]
pub enum StorageConfig {
    Local { path: PathBuf },
    S3(S3Config),
    Memory,
}

#[derive(Debug, Clone)]
pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub endpoint: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub path_style: bool,
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// Origins allowed to call the API cross-origin (desktop app, separately hosted web app).
    pub cors_allowed_origins: Vec<String>,
    /// Trust `X-Forwarded-For` from a reverse proxy when identifying the client IP.
    pub trust_proxy: bool,
    pub request_timeout: Duration,
    pub max_body_bytes: usize,
    /// When set, every API request must send it in the `x-api-key` header. The desktop app's local
    /// server sets a new random key on each start, so only the app's own window can use it.
    pub api_key: Option<String>,
}

impl Config {
    /// Reads configuration from the process environment (after loading a local env file, if any).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| {
            std::env::var(key)
                .ok()
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        })
    }

    /// Builds configuration from any key lookup; used by tests.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let environment = match lookup("APP_ENV").as_deref() {
            None | Some("development") => Environment::Development,
            Some("test") => Environment::Test,
            Some("production") => Environment::Production,
            Some(other) => bail!("APP_ENV must be development, test or production, got {other:?}"),
        };
        let brand = Brand::load(&lookup)?;
        let host = lookup("API_HOST").unwrap_or_else(|| "0.0.0.0".into());
        let port: u16 = parse(&lookup, "API_PORT", 8000)?;
        let bind_addr = SocketAddr::from_str(&format!("{host}:{port}"))
            .context("API_HOST/API_PORT do not form a socket address")?;

        let database = DatabaseConfig {
            url: lookup("DATABASE_URL").context("DATABASE_URL is required")?,
            max_connections: parse(&lookup, "DATABASE_MAX_CONNECTIONS", 10)?,
            acquire_timeout: Duration::from_secs(parse(
                &lookup,
                "DATABASE_ACQUIRE_TIMEOUT_SECONDS",
                5,
            )?),
        };

        let email = match lookup("EMAIL_PROVIDER").as_deref() {
            None | Some("smtp") => EmailConfig::Smtp(SmtpConfig {
                host: lookup("SMTP_HOST").unwrap_or_else(|| "localhost".into()),
                port: parse(&lookup, "SMTP_PORT", 1025)?,
                username: lookup("SMTP_USERNAME"),
                password: lookup("SMTP_PASSWORD"),
                tls: match lookup("SMTP_TLS").as_deref() {
                    None | Some("none") => SmtpTls::None,
                    Some("starttls") => SmtpTls::StartTls,
                    Some("tls") => SmtpTls::Tls,
                    Some(other) => bail!("SMTP_TLS must be none, starttls or tls, got {other:?}"),
                },
            }),
            Some("log") => EmailConfig::Log,
            Some("memory") => EmailConfig::Memory,
            Some(other) => bail!("EMAIL_PROVIDER must be smtp, log or memory, got {other:?}"),
        };

        let storage = match lookup("STORAGE_BACKEND").as_deref() {
            None | Some("local") => StorageConfig::Local {
                path: PathBuf::from(
                    lookup("STORAGE_LOCAL_PATH").unwrap_or_else(|| "./data/storage".into()),
                ),
            },
            Some("s3") => StorageConfig::S3(S3Config {
                bucket: lookup("S3_BUCKET")
                    .context("S3_BUCKET is required when STORAGE_BACKEND=s3")?,
                region: lookup("S3_REGION").unwrap_or_else(|| "us-east-1".into()),
                endpoint: lookup("S3_ENDPOINT"),
                access_key_id: lookup("S3_ACCESS_KEY_ID"),
                secret_access_key: lookup("S3_SECRET_ACCESS_KEY"),
                path_style: parse(&lookup, "S3_FORCE_PATH_STYLE", false)?,
            }),
            Some("memory") => StorageConfig::Memory,
            Some(other) => bail!("STORAGE_BACKEND must be local, s3 or memory, got {other:?}"),
        };

        let http = HttpConfig {
            cors_allowed_origins: lookup("CORS_ALLOWED_ORIGINS")
                .map(|v| {
                    v.split(',')
                        .map(|s| s.trim().trim_end_matches('/').to_owned())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            trust_proxy: parse(&lookup, "TRUST_PROXY", false)?,
            request_timeout: Duration::from_secs(parse(
                &lookup,
                "API_REQUEST_TIMEOUT_SECONDS",
                30,
            )?),
            max_body_bytes: 1024 * 1024 * parse::<usize>(&lookup, "API_MAX_UPLOAD_MB", 10)?,
            api_key: lookup("API_KEY"),
        };

        if environment == Environment::Production
            && (matches!(email, EmailConfig::Memory) || matches!(storage, StorageConfig::Memory))
        {
            bail!("memory email or storage backends are for tests only");
        }

        Ok(Self {
            environment,
            bind_addr,
            database,
            email,
            storage,
            http,
            brand,
        })
    }
}

impl Config {
    /// Whether the API can actually send email (the `log` provider only writes it to the log).
    pub fn email_enabled(&self) -> bool {
        !matches!(self.email, EmailConfig::Log)
    }
}

fn parse<T: FromStr>(
    lookup: &impl Fn(&str) -> Option<String>,
    key: &str,
    default: T,
) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    match lookup(key) {
        None => Ok(default),
        Some(raw) => raw
            .parse()
            .map_err(|e| anyhow::anyhow!("{key} is invalid ({raw:?}): {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()))
    }

    #[test]
    fn minimal_configuration_uses_safe_defaults() {
        let cfg = config(&[("DATABASE_URL", "postgres://localhost/solo")]).expect("valid");
        assert_eq!(cfg.bind_addr.port(), 8000);
        assert!(cfg.http.api_key.is_none());
        assert!(matches!(cfg.storage, StorageConfig::Local { .. }));
    }

    #[test]
    fn an_api_key_can_be_required() {
        let cfg = config(&[("DATABASE_URL", "postgres://x/y"), ("API_KEY", "k")]).expect("valid");
        assert_eq!(cfg.http.api_key.as_deref(), Some("k"));
    }

    #[test]
    fn database_url_is_required() {
        assert!(config(&[]).is_err());
    }

    #[test]
    fn invalid_values_are_reported() {
        assert!(config(&[("DATABASE_URL", "postgres://x/y"), ("API_PORT", "eighty")]).is_err());
        assert!(
            config(&[
                ("DATABASE_URL", "postgres://x/y"),
                ("EMAIL_PROVIDER", "carrier-pigeon")
            ])
            .is_err()
        );
    }
}
