//! The owner's email settings (SL-015): their own mail account (SMTP), used to send invoices, quotes,
//! statements and reminders when the server has no mail server of its own (the desktop app).

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::config::{SmtpConfig, SmtpTls};
use crate::context::Owner;
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::services::email::smtp::SmtpEmailSender;
use crate::services::email::{EmailAddress, EmailMessage, Mailer, templates};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SmtpSecurity {
    /// No encryption (only for a mail server on this computer or network).
    None,
    /// STARTTLS, usually port 587.
    Starttls,
    /// TLS from the start, usually port 465.
    Tls,
}

impl SmtpSecurity {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Starttls => "starttls",
            Self::Tls => "tls",
        }
    }
    fn parse(value: &str) -> Self {
        match value {
            "none" => Self::None,
            "tls" => Self::Tls,
            _ => Self::Starttls,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct Row {
    enabled: bool,
    host: String,
    port: i32,
    security: String,
    username: String,
    password: String,
    from_name: String,
    from_address: String,
    tested_at: Option<DateTime<Utc>>,
    test_error: Option<String>,
}

/// How email is sent, without the password.
#[derive(Debug, Serialize, ToSchema)]
pub struct EmailSettings {
    /// The server has its own mail server (development and tests): these settings aren't used.
    pub server_managed: bool,
    /// Settings have been saved.
    pub configured: bool,
    pub enabled: bool,
    pub host: String,
    pub port: i32,
    pub security: SmtpSecurity,
    pub username: String,
    /// A password is saved (it's never sent back).
    pub has_password: bool,
    pub from_name: String,
    pub from_address: String,
    pub tested_at: Option<DateTime<Utc>>,
    /// Why the last test failed, if it did.
    pub test_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct EmailSettingsInput {
    #[serde(default = "yes")]
    pub enabled: bool,
    pub host: String,
    pub port: i32,
    pub security: SmtpSecurity,
    #[serde(default)]
    pub username: String,
    /// Leave empty to keep the saved password.
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub from_name: String,
    pub from_address: String,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TestEmailInput {
    /// Where to send the test (defaults to the sender address).
    #[serde(default)]
    pub to: Option<String>,
}

async fn find(conn: &mut PgConnection, user_id: Uuid) -> Result<Option<Row>, sqlx::Error> {
    sqlx::query_as(
        "SELECT enabled, host, port, security, username, password, from_name, from_address, tested_at, test_error
         FROM email_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await
}

fn view(state: &AppState, row: Option<Row>) -> EmailSettings {
    let server_managed = state.config.email_enabled();
    match row {
        Some(r) => EmailSettings {
            server_managed,
            configured: true,
            enabled: r.enabled,
            host: r.host,
            port: r.port,
            security: SmtpSecurity::parse(&r.security),
            username: r.username,
            has_password: !r.password.is_empty(),
            from_name: r.from_name,
            from_address: r.from_address,
            tested_at: r.tested_at,
            test_error: r.test_error,
        },
        None => EmailSettings {
            server_managed,
            configured: false,
            enabled: false,
            host: String::new(),
            port: 587,
            security: SmtpSecurity::Starttls,
            username: String::new(),
            has_password: false,
            from_name: String::new(),
            from_address: String::new(),
            tested_at: None,
            test_error: None,
        },
    }
}

fn mailer_of(row: &Row) -> AppResult<Mailer> {
    let config = SmtpConfig {
        host: row.host.clone(),
        port: row.port as u16,
        username: (!row.username.is_empty()).then(|| row.username.clone()),
        password: (!row.password.is_empty()).then(|| row.password.clone()),
        tls: match SmtpSecurity::parse(&row.security) {
            SmtpSecurity::None => SmtpTls::None,
            SmtpSecurity::Starttls => SmtpTls::StartTls,
            SmtpSecurity::Tls => SmtpTls::Tls,
        },
    };
    let sender = SmtpEmailSender::new(&config).map_err(|e| {
        AppError::validation("host", &format!("This mail server can’t be used: {e}"))
    })?;
    Ok(Mailer::with_from(
        Arc::new(sender),
        EmailAddress::new(row.from_name.clone(), row.from_address.clone()),
    ))
}

/// The mailer to send with: the server's own when it has one, else the owner's saved settings when
/// they're on. None when email isn't set up.
pub async fn mailer(state: &AppState, user_id: Uuid) -> AppResult<Option<Mailer>> {
    if state.config.email_enabled() {
        return Ok(Some(state.mailer.clone()));
    }
    let mut tx = state.db.begin_user(user_id).await?;
    let row = find(&mut tx, user_id).await?;
    tx.commit().await?;
    match row {
        Some(r) if r.enabled => Ok(Some(mailer_of(&r)?)),
        _ => Ok(None),
    }
}

/// Whether email can be sent: the server has a mail server, or the owner turned theirs on.
pub async fn enabled(state: &AppState) -> AppResult<bool> {
    if state.config.email_enabled() {
        return Ok(true);
    }
    let mut tx = state.db.begin().await?;
    let on: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM email_settings WHERE enabled)")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(on)
}

pub async fn get(state: &AppState, owner: &Owner) -> AppResult<EmailSettings> {
    let mut tx = state.db.begin_user(owner.user_id).await?;
    let row = find(&mut tx, owner.user_id).await?;
    tx.commit().await?;
    Ok(view(state, row))
}

pub async fn save(
    state: &AppState,
    owner: &Owner,
    input: EmailSettingsInput,
) -> AppResult<EmailSettings> {
    let mut errors = FieldErrors::new();
    let host = input.host.trim().to_owned();
    errors.check(
        !host.is_empty() && host.len() <= 253 && !host.contains(char::is_whitespace),
        "host",
        "Enter the mail server, e.g. smtp.gmail.com.",
    );
    errors.check(
        (1..=65535).contains(&input.port),
        "port",
        "Enter a port between 1 and 65535 (usually 587 or 465).",
    );
    let from_address = validation::normalize_email(&input.from_address);
    errors.check(
        validation::is_valid_email(&from_address),
        "from_address",
        "Enter the email address to send from.",
    );
    let username = input.username.trim().to_owned();
    errors.check(username.len() <= 320, "username", "This is too long.");
    let from_name = input.from_name.trim().to_owned();
    errors.check(
        from_name.chars().count() <= 120,
        "from_name",
        "Use 120 characters or fewer.",
    );
    if let Some(p) = &input.password {
        errors.check(p.len() <= 500, "password", "This is too long.");
    }
    errors.into_result()?;
    let mut tx = state.db.begin_user(owner.user_id).await?;
    // An empty password keeps the saved one.
    let password = input.password.filter(|p| !p.is_empty());
    sqlx::query(
        "INSERT INTO email_settings (user_id, enabled, host, port, security, username, password, from_name, from_address)
         VALUES ($1, $2, $3, $4, $5, $6, coalesce($7, ''), $8, $9)
         ON CONFLICT (user_id) DO UPDATE SET enabled = EXCLUDED.enabled, host = EXCLUDED.host, port = EXCLUDED.port,
           security = EXCLUDED.security, username = EXCLUDED.username,
           password = coalesce($7, email_settings.password), from_name = EXCLUDED.from_name,
           from_address = EXCLUDED.from_address, tested_at = NULL, test_error = NULL, updated_at = now()",
    )
    .bind(owner.user_id)
    .bind(input.enabled)
    .bind(&host)
    .bind(input.port)
    .bind(input.security.as_str())
    .bind(&username)
    .bind(password)
    .bind(&from_name)
    .bind(&from_address)
    .execute(&mut *tx)
    .await?;
    let row = find(&mut tx, owner.user_id).await?;
    tx.commit().await?;
    Ok(view(state, row))
}

pub async fn delete(state: &AppState, owner: &Owner) -> AppResult<()> {
    let mut tx = state.db.begin_user(owner.user_id).await?;
    sqlx::query("DELETE FROM email_settings WHERE user_id = $1")
        .bind(owner.user_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Sends a test email with the saved settings and records the outcome. A failure comes back as a
/// conflict with the mail server's own words, so the user can fix the settings.
pub async fn test(
    state: &AppState,
    owner: &Owner,
    input: TestEmailInput,
) -> AppResult<EmailSettings> {
    let mut tx = state.db.begin_user(owner.user_id).await?;
    let row = find(&mut tx, owner.user_id)
        .await?
        .ok_or_else(|| AppError::Conflict("Save your email settings first.".into()))?;
    tx.commit().await?;
    let to = validation::normalize_email(input.to.as_deref().unwrap_or(&row.from_address));
    if !validation::is_valid_email(&to) {
        return Err(AppError::validation(
            "to",
            "Enter an email address to send the test to.",
        ));
    }
    let mailer = mailer_of(&row)?;
    let brand = &state.config.brand;
    let (html, text) = templates::render(
        brand,
        &templates::Content {
            heading: "Email is working".into(),
            paragraphs: vec![format!(
                "This test came from {} through {}. Invoices and statements you send will arrive the same way.",
                brand.name, row.host
            )],
            button: None,
            note: None,
        },
        None,
    );
    let message = EmailMessage {
        to: EmailAddress::new(String::new(), to),
        subject: format!("{} test email", brand.name),
        html,
        text,
        reply_to: None,
        attachments: Vec::new(),
    };
    let outcome = tokio::time::timeout(Duration::from_secs(30), mailer.send(message)).await;
    let error = match outcome {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e.to_string()),
        Err(_) => Some("the mail server didn’t answer within 30 seconds".to_owned()),
    };
    let mut tx = state.db.begin_user(owner.user_id).await?;
    sqlx::query("UPDATE email_settings SET tested_at = now(), test_error = $2 WHERE user_id = $1")
        .bind(owner.user_id)
        .bind(&error)
        .execute(&mut *tx)
        .await?;
    let row = find(&mut tx, owner.user_id).await?;
    tx.commit().await?;
    if let Some(error) = error {
        return Err(AppError::Conflict(format!(
            "The test email couldn’t be sent: {error}. Check the server, port, security and your user name and password (many providers need an app password)."
        )));
    }
    Ok(view(state, row))
}
