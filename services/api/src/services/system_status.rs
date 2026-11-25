//! What's working on this installation (Settings › System status): the database, file storage, PDF
//! documents and email, each with a plain-language status and what to do about it.

use std::time::{Duration, Instant};

use serde::Serialize;
use utoipa::ToSchema;

use crate::context::Owner;
use crate::domain::reports::Report;
use crate::services::{email_settings, report_export};
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    /// Working.
    Ok,
    /// Works, but something isn't set up or hasn't been checked.
    Warning,
    /// Not working.
    Error,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatusCheck {
    /// `database`, `storage`, `documents` or `email`.
    pub key: String,
    pub name: String,
    pub status: CheckStatus,
    /// What was found, in plain language.
    pub detail: String,
    /// What to do about a warning or error.
    pub action: Option<String>,
    /// The app screen that fixes it, if one does.
    pub link: Option<String>,
    /// How long the check took.
    pub took_ms: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemStatus {
    pub version: &'static str,
    /// The latest database migration the app includes.
    pub schema_version: i32,
    pub checks: Vec<StatusCheck>,
}

fn check(
    key: &str,
    name: &str,
    started: Instant,
    status: CheckStatus,
    detail: String,
    action: Option<&str>,
    link: Option<&str>,
) -> StatusCheck {
    StatusCheck {
        key: key.into(),
        name: name.into(),
        status,
        detail,
        action: action.map(Into::into),
        link: link.map(Into::into),
        took_ms: started.elapsed().as_millis() as u64,
    }
}

async fn database(state: &AppState) -> StatusCheck {
    let started = Instant::now();
    let result = async {
        let mut tx = state.db.begin().await?;
        let one: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok::<i32, sqlx::Error>(one)
    };
    match tokio::time::timeout(Duration::from_secs(5), result).await {
        Ok(Ok(_)) => check(
            "database",
            "Database",
            started,
            CheckStatus::Ok,
            "Your books are stored and answering.".into(),
            None,
            None,
        ),
        Ok(Err(e)) => check(
            "database",
            "Database",
            started,
            CheckStatus::Error,
            format!("The database didn’t answer: {e}"),
            Some("Restart the app. If it keeps happening, restart the computer."),
            None,
        ),
        Err(_) => check(
            "database",
            "Database",
            started,
            CheckStatus::Error,
            "The database took more than 5 seconds to answer.".into(),
            Some("Restart the app."),
            None,
        ),
    }
}

async fn storage(state: &AppState) -> StatusCheck {
    let started = Instant::now();
    let key = format!("system-status/{}.txt", uuid::Uuid::now_v7());
    let result = async {
        state.storage.put(&key, b"status check".to_vec()).await?;
        let back = state.storage.get(&key).await?;
        state.storage.delete(&key).await?;
        anyhow::ensure!(
            back.as_deref() == Some(&b"status check"[..]),
            "a test file read back differently"
        );
        Ok(())
    };
    match result.await {
        Ok(()) => check(
            "storage",
            "Receipts and files",
            started,
            CheckStatus::Ok,
            "Receipts can be saved and opened.".into(),
            None,
            None,
        ),
        Err(e) => check(
            "storage",
            "Receipts and files",
            started,
            CheckStatus::Error,
            format!("Files can’t be saved: {e}"),
            Some("Check the disk isn’t full and that the app’s data folder can be written to."),
            None,
        ),
    }
}

async fn documents(state: &AppState) -> StatusCheck {
    let started = Instant::now();
    let report = Report {
        kind: "status".into(),
        title: "Status check".into(),
        business_name: state.config.brand.name.clone(),
        currency: "NZD".into(),
        period: None,
        basis: None,
        subtitle: Vec::new(),
        sections: Vec::new(),
        notes: vec!["A test page.".into()],
        generated_on: chrono::Utc::now().date_naive(),
    };
    let brand = &state.config.brand;
    match report_export::to_pdf(&report, &brand.primary_color, &brand.name).await {
        Ok(bytes) if bytes.starts_with(b"%PDF") => check(
            "documents",
            "PDF documents",
            started,
            CheckStatus::Ok,
            "Invoices, quotes and reports can be made as PDFs.".into(),
            None,
            None,
        ),
        Ok(_) | Err(_) => check(
            "documents",
            "PDF documents",
            started,
            CheckStatus::Error,
            "A test PDF couldn’t be made.".into(),
            Some("Reinstall the app to restore its document templates and fonts."),
            None,
        ),
    }
}

async fn email(state: &AppState, owner: &Owner) -> StatusCheck {
    let started = Instant::now();
    let link = Some("/settings/email");
    let settings = match email_settings::get(state, owner).await {
        Ok(s) => s,
        Err(_) => {
            return check(
                "email",
                "Email",
                started,
                CheckStatus::Error,
                "The email settings couldn’t be read.".into(),
                None,
                link,
            );
        }
    };
    if settings.server_managed {
        return check(
            "email",
            "Email",
            started,
            CheckStatus::Ok,
            "This server sends email itself.".into(),
            None,
            None,
        );
    }
    let (status, detail, action) = if !settings.configured {
        (
            CheckStatus::Warning,
            "Not set up: invoices and statements are downloaded and sent from your own email."
                .to_owned(),
            Some("To send them from the app, add your email account and send a test email."),
        )
    } else if !settings.enabled {
        (
            CheckStatus::Warning,
            format!("Set up for {} but turned off.", settings.from_address),
            Some("Turn email on in Settings › Email."),
        )
    } else if let Some(error) = &settings.test_error {
        (
            CheckStatus::Error,
            format!("The last test failed: {error}"),
            Some("Check the settings and send another test email."),
        )
    } else if settings.tested_at.is_none() {
        (
            CheckStatus::Warning,
            format!(
                "Sending as {} through {}, not tested yet.",
                settings.from_address, settings.host
            ),
            Some("Send a test email to make sure it works."),
        )
    } else {
        (
            CheckStatus::Ok,
            format!(
                "Sending as {} through {}.",
                settings.from_address, settings.host
            ),
            None,
        )
    };
    check("email", "Email", started, status, detail, action, link)
}

pub async fn status(state: &AppState, owner: &Owner) -> SystemStatus {
    let (database, storage, documents, email) = tokio::join!(
        database(state),
        storage(state),
        documents(state),
        email(state, owner)
    );
    SystemStatus {
        version: env!("CARGO_PKG_VERSION"),
        schema_version: crate::migrations::latest_version(),
        checks: vec![database, storage, documents, email],
    }
}
