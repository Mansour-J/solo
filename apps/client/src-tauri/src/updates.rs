//! In-app updates (ADR 0015): Settings › Updates checks the release feed, then downloads, verifies and
//! installs a signed update and restarts the app on it.
//!
//! The feed's address is `plugins.updater.endpoints`, which release builds get from the `UPDATER_ENDPOINT`
//! repository variable. A build without one doesn't offer updates, and the settings page stays hidden.
//! Updates are verified against the public key in `tauri.conf.json` before they're installed.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Error as UpdaterError, Update, UpdaterExt};

use crate::backend::Backend;

/// The update found by the last check, kept until it's installed (so a failed attempt can be retried).
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

/// Whether this build can update itself, and its version.
#[derive(Debug, Serialize)]
pub struct UpdateSupport {
    pub enabled: bool,
    pub current_version: String,
}

/// A newer version on the feed.
#[derive(Debug, Serialize)]
pub struct AvailableUpdate {
    pub version: String,
    pub current_version: String,
    /// When it was published (Unix seconds), if the feed says.
    pub published_at: Option<i64>,
    /// The release notes, if any.
    pub notes: Option<String>,
}

/// Sent to the page while an update installs.
#[derive(Debug, Clone, Serialize)]
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

/// True when the updater configuration names at least one feed.
fn has_feed(updater_config: Option<&serde_json::Value>) -> bool {
    updater_config
        .and_then(|config| config.get("endpoints"))
        .and_then(|endpoints| endpoints.as_array())
        .is_some_and(|endpoints| !endpoints.is_empty())
}

fn feed_configured(app: &AppHandle) -> bool {
    has_feed(app.config().plugins.0.get("updater"))
}

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

#[tauri::command]
pub fn update_support(app: AppHandle) -> UpdateSupport {
    UpdateSupport {
        enabled: feed_configured(&app),
        current_version: app.package_info().version.to_string(),
    }
}

/// Asks the feed for a newer version; `None` when this one is the latest.
#[tauri::command]
pub async fn update_check(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<AvailableUpdate>, String> {
    if !feed_configured(&app) {
        return Err("Updates aren’t available in this version of the app.".into());
    }
    let handle = app.clone();
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        // Windows installers close the app themselves: stop the local database first.
        .on_before_exit(move || handle.state::<Backend>().stop())
        .build()
        .map_err(|e| failure(CHECK_FAILED, e))?;
    let update = updater
        .check()
        .await
        .map_err(|e| failure(CHECK_FAILED, e))?;
    let available = update.as_ref().map(|u| AvailableUpdate {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        published_at: u.date.map(|d| d.unix_timestamp()),
        notes: u.body.clone().filter(|notes| !notes.trim().is_empty()),
    });
    *pending.0.lock().expect("update lock") = update;
    Ok(available)
}

/// Downloads, verifies and installs the update found by the last check, then restarts on it.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
    on_progress: Channel<InstallProgress>,
) -> Result<(), String> {
    let update = pending
        .0
        .lock()
        .expect("update lock")
        .clone()
        .ok_or_else(|| "Check for updates first.".to_string())?;
    let (mut downloaded, mut reported) = (0_u64, 0_u64);
    let finished = on_progress.clone();
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                if reported == 0 || downloaded - reported >= PROGRESS_STEP {
                    reported = downloaded.max(1);
                    let _ = on_progress.send(InstallProgress::Downloading { downloaded, total });
                }
            },
            move || {
                let _ = finished.send(InstallProgress::Installing);
            },
        )
        .await
        .map_err(install_failure)?;
    // The new version is in place (on Windows the installer has already closed the app). Stop the
    // local database cleanly, then start the new version.
    app.state::<Backend>().stop();
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_are_offered_only_when_the_build_names_a_feed() {
        assert!(!has_feed(None), "no updater configuration");
        assert!(!has_feed(Some(&serde_json::json!({ "pubkey": "key" }))));
        assert!(!has_feed(Some(
            &serde_json::json!({ "pubkey": "key", "endpoints": [] })
        )));
        assert!(has_feed(Some(&serde_json::json!({
            "pubkey": "key",
            "endpoints": ["https://releases.example.com/latest.json"]
        }))));
    }

    #[test]
    fn failures_explain_what_to_do_without_internal_detail() {
        let check = failure(CHECK_FAILED, "dns error: no such host releases.internal");
        assert_eq!(check, CHECK_FAILED);
        assert!(!check.contains("releases.internal"));
        assert!(!failure(INSTALL_FAILED, "signature mismatch").contains("signature"));
        assert_eq!(
            install_failure(UpdaterError::SignedVersionMismatch {
                signed: "0.3.0".into(),
                announced: "0.3.1".into()
            }),
            NOT_GENUINE
        );
        assert_eq!(
            install_failure(UpdaterError::AuthenticationFailed),
            NOT_AUTHORISED
        );
        assert_eq!(
            install_failure(UpdaterError::Network("reset".into())),
            DOWNLOAD_FAILED
        );
        assert_eq!(
            install_failure(UpdaterError::TempDirNotFound),
            INSTALL_FAILED
        );
        let progress = serde_json::to_value(InstallProgress::Downloading {
            downloaded: 10,
            total: Some(20),
        })
        .expect("json");
        assert_eq!(
            progress,
            serde_json::json!({ "event": "downloading", "downloaded": 10, "total": 20 })
        );
        assert_eq!(
            serde_json::to_value(InstallProgress::Installing).expect("json"),
            serde_json::json!({ "event": "installing" })
        );
    }
}
