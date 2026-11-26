//! Backups of the books on this computer (SL-013), for the API's `services::backups`.
//!
//! A backup is one tar file:
//! - `backup.json`: what made it, the schema version and when ([`Manifest`])
//! - `database.dump`: the whole database, from PostgreSQL's own `pg_dump` (custom format)
//! - `storage/`: the receipts and other files.
//!
//! Restoring checks the file first (a Solo backup this version can open, and a readable dump), keeps
//! a copy of the current books in `<data_dir>/backups/` (the last three), then replaces the database
//! and the files and brings an older backup's schema up to date. If replacing the database fails, the
//! current books are put back. Only one backup or restore runs at a time.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use solo_api::services::backups::{BackupFile, BackupProvider, BoxFuture, RestoreError};
use sqlx::{Connection, PgConnection};
use tokio::process::Command;
use tokio::sync::Mutex;

/// The backup file layout this version writes and reads.
const FORMAT: u32 = 1;
const MANIFEST: &str = "backup.json";
const DUMP: &str = "database.dump";
const STORAGE: &str = "storage";
/// Copies of the books kept before each restore.
const KEEP_BEFORE_RESTORE: usize = 3;

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: u32,
    app: String,
    app_version: String,
    /// The highest migration the database had.
    schema_version: i32,
    made_at: DateTime<Utc>,
}

pub(crate) struct LocalBackups {
    bin_dir: PathBuf,
    port: u16,
    owner: String,
    password: String,
    database: String,
    storage: PathBuf,
    /// `<data_dir>/backups`: work folders, uploads and the copies kept before a restore.
    dir: PathBuf,
    lock: Mutex<()>,
}

fn invalid(message: &str) -> RestoreError {
    RestoreError::Invalid(message.to_owned())
}

impl LocalBackups {
    pub(crate) fn new(
        bin_dir: PathBuf,
        port: u16,
        owner: &str,
        password: &str,
        database: &str,
        storage: PathBuf,
        data_dir: &Path,
    ) -> Self {
        Self {
            bin_dir,
            port,
            owner: owner.to_owned(),
            password: password.to_owned(),
            database: database.to_owned(),
            storage,
            dir: data_dir.join("backups"),
            lock: Mutex::new(()),
        }
    }

    fn tool(&self, name: &str) -> Command {
        let mut command = Command::new(
            self.bin_dir
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
        );
        command
            .env("PGPASSWORD", &self.password)
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &self.port.to_string(),
                "-U",
                &self.owner,
            ])
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        command
    }

    async fn run(mut command: Command, what: &str) -> Result<()> {
        let output = command
            .output()
            .await
            .with_context(|| format!("running {what}"))?;
        if !output.status.success() {
            bail!(
                "{what} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    async fn dump(&self, to: &Path) -> Result<()> {
        let mut command = self.tool("pg_dump");
        command
            .args(["-d", &self.database, "--format=custom", "-f"])
            .arg(to);
        Self::run(command, "pg_dump").await
    }

    /// A new, empty work folder under `backups/`.
    fn work_dir(&self, prefix: &str) -> Result<PathBuf> {
        let dir = self.dir.join(format!(
            "{prefix}-{}",
            Alphanumeric.sample_string(&mut rand::rng(), 12)
        ));
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        Ok(dir)
    }

    /// Dumps the database and packs it with the files into `dest`.
    async fn write_backup(&self, work: &Path, dest: &Path) -> Result<DateTime<Utc>> {
        let dump = work.join(DUMP);
        self.dump(&dump).await?;
        let manifest = Manifest {
            format: FORMAT,
            app: "solo".into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            schema_version: solo_api::migrations::latest_version(),
            made_at: Utc::now(),
        };
        let made_at = manifest.made_at;
        let json = serde_json::to_vec_pretty(&manifest)?;
        let (dest, storage) = (dest.to_path_buf(), self.storage.clone());
        tokio::task::spawn_blocking(move || pack(&dest, &json, &dump, &storage)).await??;
        Ok(made_at)
    }

    async fn admin(&self) -> Result<PgConnection> {
        let url = format!(
            "postgres://{}:{}@127.0.0.1:{}/postgres",
            self.owner, self.password, self.port
        );
        PgConnection::connect(&url)
            .await
            .context("connecting to the database")
    }

    /// Replaces the database with the dump at `dump` and brings its schema up to date.
    async fn replace_database(&self, dump: &Path) -> Result<()> {
        let mut admin = self.admin().await?;
        // The database name is our own constant, so it can't break out of the statement.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            self.database
        )))
        .execute(&mut admin)
        .await?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "CREATE DATABASE {}",
            self.database
        )))
        .execute(&mut admin)
        .await?;
        admin.close().await?;
        let mut command = self.tool("pg_restore");
        command
            .args([
                "-d",
                &self.database,
                "--exit-on-error",
                "--single-transaction",
            ])
            .arg(dump);
        Self::run(command, "pg_restore").await?;
        let url = format!(
            "postgres://{}:{}@127.0.0.1:{}/{}",
            self.owner, self.password, self.port, self.database
        );
        let mut owner = PgConnection::connect(&url).await?;
        let report = solo_api::migrations::migrate(&mut owner, "solo-local restore")
            .await
            .context("updating the restored database")?;
        tracing::info!(applied = ?report.applied, "restored database brought up to date");
        owner.close().await?;
        Ok(())
    }

    /// Deletes all but the newest copies kept before a restore.
    fn prune_before_restore(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut kept: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("before-restore-"))
            })
            .collect();
        // The names sort by date.
        kept.sort();
        let excess = kept.len().saturating_sub(KEEP_BEFORE_RESTORE);
        for old in kept.into_iter().take(excess) {
            let _ = std::fs::remove_file(old);
        }
    }

    async fn restore_from(&self, file: &Path, work: &Path) -> Result<DateTime<Utc>, RestoreError> {
        // 1. Check the file before touching anything.
        let unpacked = work.join("backup");
        let (from, to) = (file.to_path_buf(), unpacked.clone());
        tokio::task::spawn_blocking(move || unpack(&from, &to))
            .await
            .map_err(|e| RestoreError::Failed(e.into()))?
            .map_err(|_| invalid("This isn’t a Solo backup file, or it’s damaged."))?;
        let manifest: Manifest = std::fs::read(unpacked.join(MANIFEST))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .ok_or_else(|| invalid("This isn’t a Solo backup file, or it’s damaged."))?;
        if manifest.app != "solo" || manifest.format > FORMAT {
            return Err(invalid(
                "This backup was made by a newer version of Solo. Update Solo, then restore it.",
            ));
        }
        if manifest.schema_version > solo_api::migrations::latest_version() {
            return Err(RestoreError::Invalid(format!(
                "This backup was made by a newer version of Solo ({}). Update Solo, then restore it.",
                manifest.app_version
            )));
        }
        let dump = unpacked.join(DUMP);
        let mut list = self.tool("pg_restore");
        list.arg("--list").arg(&dump);
        if Self::run(list, "pg_restore --list").await.is_err() {
            return Err(invalid(
                "This backup’s database is damaged, so it can’t be restored.",
            ));
        }

        // 2. Keep the current books.
        let current = work.join("current");
        std::fs::create_dir_all(&current).map_err(|e| RestoreError::Failed(e.into()))?;
        let kept = self.dir.join(format!(
            "before-restore-{}.solobackup",
            Utc::now().format("%Y%m%d-%H%M%S")
        ));
        self.write_backup(&current, &kept)
            .await
            .map_err(RestoreError::Failed)?;
        self.prune_before_restore();

        // 3. Replace the database, putting the current one back if that fails.
        if let Err(error) = self.replace_database(&dump).await {
            tracing::error!("restoring the backup failed, putting the books back: {error:#}");
            if let Err(again) = self.replace_database(&current.join(DUMP)).await {
                tracing::error!(
                    "putting the books back failed too: {again:#}; a copy is in {}",
                    kept.display()
                );
            }
            return Err(RestoreError::Failed(error));
        }

        // 4. Swap the files.
        let restored_files = unpacked.join(STORAGE);
        let old_files = work.join("old-storage");
        if self.storage.exists() {
            std::fs::rename(&self.storage, &old_files)
                .map_err(|e| RestoreError::Failed(e.into()))?;
        }
        let moved = if restored_files.exists() {
            std::fs::rename(&restored_files, &self.storage)
        } else {
            std::fs::create_dir_all(&self.storage)
        };
        if let Err(error) = moved {
            let _ = std::fs::rename(&old_files, &self.storage);
            return Err(RestoreError::Failed(error.into()));
        }
        Ok(manifest.made_at)
    }
}

/// Packs the manifest, the dump and the files folder into a tar file at `dest`.
fn pack(dest: &Path, manifest: &[u8], dump: &Path, storage: &Path) -> Result<()> {
    let file =
        std::fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let mut tar = tar::Builder::new(std::io::BufWriter::new(file));
    tar.follow_symlinks(false);
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(Utc::now().timestamp().max(0) as u64);
    header.set_cksum();
    tar.append_data(&mut header, MANIFEST, manifest)?;
    tar.append_path_with_name(dump, DUMP)?;
    if storage.exists() {
        tar.append_dir_all(STORAGE, storage)?;
    }
    tar.into_inner()?
        .into_inner()
        .map_err(|e| e.into_error())?
        .sync_all()?;
    Ok(())
}

/// Unpacks a backup into `to`, refusing anything that would land outside it.
fn unpack(file: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    let mut archive = tar::Archive::new(std::fs::File::open(file)?);
    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.unpack_in(to)? {
            bail!("an entry would be written outside the backup folder");
        }
    }
    Ok(())
}

impl BackupProvider for LocalBackups {
    fn backup(&self) -> BoxFuture<'_, Result<BackupFile>> {
        Box::pin(async move {
            let _one_at_a_time = self.lock.lock().await;
            let work = self.work_dir("backup")?;
            let dest = self.dir.join(format!(
                "backup-{}.solobackup",
                Alphanumeric.sample_string(&mut rand::rng(), 12)
            ));
            let result = self.write_backup(&work, &dest).await;
            let _ = std::fs::remove_dir_all(&work);
            let made_at = match result {
                Ok(made_at) => made_at,
                Err(error) => {
                    let _ = std::fs::remove_file(&dest);
                    return Err(error);
                }
            };
            let size_bytes = std::fs::metadata(&dest)?.len();
            Ok(BackupFile {
                path: dest,
                made_at,
                size_bytes,
            })
        })
    }

    fn restore(&self, file: PathBuf) -> BoxFuture<'_, Result<DateTime<Utc>, RestoreError>> {
        Box::pin(async move {
            let _one_at_a_time = self.lock.lock().await;
            let work = self.work_dir("restore").map_err(RestoreError::Failed)?;
            let result = self.restore_from(&file, &work).await;
            let _ = std::fs::remove_dir_all(&work);
            result
        })
    }

    fn upload_dir(&self) -> PathBuf {
        self.dir.join("uploads")
    }
}
