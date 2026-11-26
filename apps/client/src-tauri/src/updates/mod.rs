//! In-app updates (ADR 0015): Settings › Updates checks the release feed, then downloads, verifies and
//! installs a signed update and restarts the app on it.
//!
//! The feed's address is `plugins.updater.endpoints`, which release builds get from the `UPDATER_ENDPOINT`
//! repository variable. A build without one doesn't offer updates, and the settings page stays hidden.
//! Updates are verified against the public key in `tauri.conf.json` before they're installed, and must be
//! signed for the version the feed announces (`requireSignedVersion`), so an old release can't be passed off
//! as a new one.
//!
//! Everything here runs in the tests (`tests/updates.rs`, against a local feed signed with a throwaway key). The
//! three steps that leave the app (stopping the local database, running the installer and restarting) are
//! behind [`System`]; the real ones are in `system.rs`.

mod system;

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::{Channel, Invoke};
use tauri::{AppHandle, Config, Runtime, State};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterBuilder, UpdaterExt};

pub use system::Desktop;

/// What updating does outside the app. [`Desktop`] in the app; a recording fake in the tests.
pub trait System: Send + Sync + 'static {
    /// Prepares the updater for an installer that closes the app itself (Windows): stop the local
    /// database first.
    fn prepare(&self, builder: UpdaterBuilder) -> UpdaterBuilder;
    /// Installs a downloaded update whose signature has been checked.
    fn install(&self, update: &Update, bytes: &[u8]) -> Result<(), UpdaterError>;
    /// Stops the local database cleanly and starts the new version.
    fn restart(&self);
}

/// The update found by the last check (kept until it's installed, so a failed attempt can be retried),
/// and the system it installs on.
pub struct Updates {
    pending: Mutex<Option<Update>>,
    system: Box<dyn System>,
}

impl Updates {
    pub fn new(system: impl System) -> Self {
        Self {
            pending: Mutex::new(None),
            system: Box::new(system),
        }
    }

    fn pending(&self) -> Option<Update> {
        self.pending.lock().expect("update lock").clone()
    }
}

/// Whether this build can update itself, and its version.
#[derive(Debug, PartialEq, Serialize)]
pub struct UpdateSupport {
    pub enabled: bool,
    pub current_version: String,
}

/// A newer version on the feed.
#[derive(Debug, PartialEq, Serialize)]
pub struct AvailableUpdate {
    pub version: String,
    pub current_version: String,
    /// When it was published (Unix seconds), if the feed says.
    pub published_at: Option<i64>,
    /// The release notes, if any.
    pub notes: Option<String>,
}

impl From<&Update> for AvailableUpdate {
    fn from(update: &Update) -> Self {
        Self {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            published_at: update.date.map(|date| date.unix_timestamp()),
            notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
        }
    }
}

/// Sent to the page while an update installs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum InstallProgress {
    Downloading {
        downloaded: u64,
        /// Total bytes, when the server reports it.
        total: Option<u64>,
    },
    /// Downloaded and verified: installing, then restarting.
    Installing,
}

/// Progress is sent at most this often (in bytes), so a fast download doesn't flood the page.
const PROGRESS_STEP: u64 = 256 * 1024;

/// Counts downloaded bytes and says when to tell the page: on the first chunk, then every
/// [`PROGRESS_STEP`] bytes.
#[derive(Default)]
struct DownloadProgress {
    downloaded: u64,
    reported: Option<u64>,
}

impl DownloadProgress {
    fn add(&mut self, chunk: usize) -> Option<u64> {
        self.downloaded += chunk as u64;
        let due = self
            .reported
            .is_none_or(|reported| self.downloaded - reported >= PROGRESS_STEP);
        due.then(|| {
            self.reported = Some(self.downloaded);
            self.downloaded
        })
    }
}

/// True when the updater configuration names at least one feed.
fn has_feed(config: &Config) -> bool {
    config
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("endpoints"))
        .and_then(|endpoints| endpoints.as_array())
        .is_some_and(|endpoints| !endpoints.is_empty())
}

const UNAVAILABLE: &str = "Updates aren’t available in this version of the app.";
const CHECK_FIRST: &str = "Check for updates first.";
const CHECK_FAILED: &str =
    "Couldn’t check for updates. Check your internet connection and try again.";
const DOWNLOAD_FAILED: &str =
    "The download didn’t finish. Check your internet connection and try again.";
const NOT_GENUINE: &str =
    "The download didn’t pass the security check, so it wasn’t installed. Try again later.";
const NOT_AUTHORISED: &str = "Installing needs your computer’s password. The update wasn’t installed because the password was cancelled or the installation failed.";
const INSTALL_FAILED: &str =
    "The update couldn’t be installed. Try again later, or install the new version yourself.";

/// Logs the cause and returns what the user should see (without internal detail).
fn failure(message: &str, error: impl std::fmt::Display) -> String {
    log::error!("{message} ({error})");
    message.to_owned()
}

/// What went wrong installing, in words the user can act on.
fn install_failure(error: UpdaterError) -> String {
    let message = match &error {
        UpdaterError::Reqwest(_) | UpdaterError::Network(_) => DOWNLOAD_FAILED,
        UpdaterError::Minisign(_)
        | UpdaterError::Base64(_)
        | UpdaterError::SignatureUtf8(_)
        | UpdaterError::SignedVersionMismatch { .. }
        | UpdaterError::MissingSignedVersion
        | UpdaterError::InvalidUpdaterFormat => NOT_GENUINE,
        UpdaterError::AuthenticationFailed
        | UpdaterError::DebInstallFailed
        | UpdaterError::PackageInstallFailed => NOT_AUTHORISED,
        _ => INSTALL_FAILED,
    };
    failure(message, error)
}

/// Every update command's name starts with this (`lib.rs` sends them to [`commands`]).
pub const PREFIX: &str = "update_";

/// The update commands' IPC handler, registered by `lib.rs` (and by the tests' app).
pub fn commands<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![update_support, update_check, update_install]
}

#[tauri::command]
pub fn update_support<R: Runtime>(app: AppHandle<R>) -> UpdateSupport {
    UpdateSupport {
        enabled: has_feed(app.config()),
        current_version: app.package_info().version.to_string(),
    }
}

/// Asks the feed for a newer version; `None` when this one is the latest.
#[tauri::command]
pub async fn update_check<R: Runtime>(
    app: AppHandle<R>,
    updates: State<'_, Updates>,
) -> Result<Option<AvailableUpdate>, String> {
    if !has_feed(app.config()) {
        return Err(UNAVAILABLE.into());
    }
    let builder = app.updater_builder().timeout(Duration::from_secs(30));
    let check = async { updates.system.prepare(builder).build()?.check().await };
    let update = check.await.map_err(|e| failure(CHECK_FAILED, e))?;
    let available = update.as_ref().map(AvailableUpdate::from);
    *updates.pending.lock().expect("update lock") = update;
    Ok(available)
}

/// Downloads and verifies the update found by the last check, installs it and restarts on it.
#[tauri::command]
pub async fn update_install(
    updates: State<'_, Updates>,
    on_progress: Channel<InstallProgress>,
) -> Result<(), String> {
    let update = updates.pending().ok_or_else(|| CHECK_FIRST.to_string())?;
    let mut progress = DownloadProgress::default();
    let bytes = update
        .download(
            |chunk, total| {
                if let Some(downloaded) = progress.add(chunk) {
                    let _ = on_progress.send(InstallProgress::Downloading { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(install_failure)?;
    let _ = on_progress.send(InstallProgress::Installing);
    updates
        .system
        .install(&update, &bytes)
        .map_err(install_failure)?;
    // The new version is in place (on Windows the installer has already closed the app).
    updates.system.restart();
    Ok(())
}
