//! Backups of the books on this computer (SL-013): one file with the database and the receipts,
//! saved where the owner chooses, and a restore that replaces the books with one.
//!
//! Only the desktop app's local server can make them (it runs PostgreSQL's own `pg_dump` and
//! `pg_restore` as the database owner), so it hands the API a [`BackupProvider`]; elsewhere
//! (development against Docker, the tests) there is none and backups aren't offered. The API records
//! each backup in `backups`, for the date on Settings › Your data, the Business health check and the
//! dashboard reminder.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::Owner;
use crate::error::{AppError, AppResult};
use crate::repositories::{audit, users};
use crate::state::AppState;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A backup file the provider wrote, for the API to send and then delete.
#[derive(Debug)]
pub struct BackupFile {
    pub path: PathBuf,
    pub made_at: DateTime<Utc>,
    pub size_bytes: u64,
}

#[derive(Debug)]
pub enum RestoreError {
    /// Not a backup this version can restore; the message is for the owner. Nothing was changed.
    Invalid(String),
    Failed(anyhow::Error),
}

/// Makes and restores backups (the desktop app's local server).
pub trait BackupProvider: Send + Sync {
    /// Writes a backup of every business and its receipts to a new file.
    fn backup(&self) -> BoxFuture<'_, anyhow::Result<BackupFile>>;
    /// Replaces the books with the backup in `file`, keeping a copy of the current books first.
    /// Returns when the backup was made. The app must restart afterwards.
    fn restore(&self, file: PathBuf) -> BoxFuture<'_, Result<DateTime<Utc>, RestoreError>>;
    /// A folder for a backup being uploaded to restore.
    fn upload_dir(&self) -> PathBuf;
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct BackupRecord {
    /// `backup` (made here) or `restored` (the backup the books were restored from).
    pub kind: String,
    /// When the backup was made.
    pub made_at: DateTime<Utc>,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupStatus {
    /// Backups can be made here (the desktop app).
    pub available: bool,
    /// The newest backup the books have (made or restored from).
    pub last: Option<BackupRecord>,
    /// The most recent, newest first (up to 5).
    pub recent: Vec<BackupRecord>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreResult {
    /// When the restored backup was made.
    pub made_at: DateTime<Utc>,
    /// The app has to restart to open the restored books.
    pub restart_required: bool,
}

async fn recent(conn: &mut PgConnection) -> Result<Vec<BackupRecord>, sqlx::Error> {
    sqlx::query_as("SELECT kind, made_at, size_bytes FROM backups ORDER BY made_at DESC LIMIT 5")
        .fetch_all(conn)
        .await
}

async fn record(
    conn: &mut PgConnection,
    kind: &str,
    made_at: DateTime<Utc>,
    size: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO backups (kind, made_at, size_bytes) VALUES ($1, $2, $3)")
        .bind(kind)
        .bind(made_at)
        .bind(size)
        .execute(conn)
        .await?;
    Ok(())
}

fn unavailable() -> AppError {
    AppError::Conflict("Backups are made by the desktop app.".into())
}

pub async fn status(state: &AppState) -> AppResult<BackupStatus> {
    let mut conn = state.db.pool().acquire().await?;
    let recent = recent(&mut conn).await?;
    Ok(BackupStatus {
        available: state.backups.is_some(),
        last: recent.first().cloned(),
        recent,
    })
}

/// When the books were last backed up, if backups are made here (`None` outside the desktop app).
pub async fn last_backup(
    state: &AppState,
    conn: &mut PgConnection,
) -> AppResult<Option<Option<DateTime<Utc>>>> {
    if state.backups.is_none() {
        return Ok(None);
    }
    let last: Option<DateTime<Utc>> = sqlx::query_scalar("SELECT max(made_at) FROM backups")
        .fetch_one(conn)
        .await?;
    Ok(Some(last))
}

/// Makes a backup and records it. The caller sends the file and deletes it.
pub async fn create(state: &AppState) -> AppResult<BackupFile> {
    let provider = state.backups.clone().ok_or_else(unavailable)?;
    // On a task of its own, so a dropped request can't stop it halfway.
    let file = tokio::spawn(async move { provider.backup().await })
        .await
        .map_err(AppError::internal)?
        .map_err(AppError::internal)?;
    let mut conn = state.db.pool().acquire().await?;
    record(
        &mut conn,
        "backup",
        file.made_at,
        i64::try_from(file.size_bytes).unwrap_or(i64::MAX),
    )
    .await?;
    tracing::info!(size = file.size_bytes, "backup made");
    Ok(file)
}

/// Where an upload to restore goes, before [`restore`].
pub fn upload_path(state: &AppState) -> AppResult<PathBuf> {
    let provider = state.backups.as_ref().ok_or_else(unavailable)?;
    let dir = provider.upload_dir();
    std::fs::create_dir_all(&dir).map_err(AppError::internal)?;
    Ok(dir.join(format!("{}.upload", Uuid::now_v7())))
}

/// Replaces the books with the uploaded backup at `file` (deleted afterwards), then records it.
pub async fn restore(state: &AppState, owner: &Owner, file: PathBuf) -> AppResult<RestoreResult> {
    let provider = state.backups.clone().ok_or_else(unavailable)?;
    let size = std::fs::metadata(&file)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    let upload = file.clone();
    let result = tokio::spawn(async move { provider.restore(upload).await })
        .await
        .map_err(AppError::internal)?;
    let _ = std::fs::remove_file(&file);
    let made_at = match result {
        Ok(made_at) => made_at,
        Err(RestoreError::Invalid(message)) => return Err(AppError::validation("file", &message)),
        Err(RestoreError::Failed(error)) => return Err(AppError::internal(error)),
    };
    // The restored books have their own owner (this one, or the computer the backup came from).
    let mut tx = state.db.begin().await?;
    let restored_owner = users::first(&mut tx).await?.map(|u| u.id);
    record(&mut tx, "restored", made_at, size).await?;
    tx.commit().await?;
    if let Some(user_id) = restored_owner {
        let mut tx = state.db.begin_user(user_id).await?;
        let mut actor = owner.actor();
        actor.user_id = Some(user_id);
        audit::record(
            &mut tx,
            &actor,
            audit::Entry {
                business_id: None,
                action: "backup.restored",
                entity_type: "user",
                entity_id: Some(user_id),
                summary: format!(
                    "Restored the books from a backup made {}",
                    made_at.format("%-d %B %Y %H:%M UTC")
                ),
                changes: json!({ "made_at": made_at }),
            },
        )
        .await?;
        tx.commit().await?;
    }
    tracing::info!(%made_at, "backup restored");
    Ok(RestoreResult {
        made_at,
        restart_required: true,
    })
}
