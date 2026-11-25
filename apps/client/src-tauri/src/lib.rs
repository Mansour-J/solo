//! Desktop shell.
//!
//! The React app owns all product logic and talks to the REST API, which runs inside this app on the
//! user's own data (`backend.rs`: the local server with its embedded PostgreSQL). Solo is for one
//! person, so there is no sign-in. The shell provides only what the web layer cannot do itself:
//!
//! * `backend_status`, `restart_app` — the local server's address and key, and recovering from a
//!   failed start (see `backend.rs`).
//! * `update_*` — check for, install and restart on a signed update (see `updates.rs`, desktop only).
//! * `opener` — open external links in the system browser.
//! * `log` — debug-build logging.

mod backend;
#[cfg(desktop)]
mod updates;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(updates::PendingUpdate::default());
    builder
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // The local (not roaming) app-data folder: the database and the PostgreSQL program are
            // large and live, so they must not sync between computers with a roaming profile.
            let data_dir = app.path().app_local_data_dir()?;
            app.manage(backend::Backend::default());
            backend::Backend::start(app.handle(), data_dir);
            #[cfg(unix)]
            exit_on_signal(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            backend::backend_status,
            backend::restart_app,
            #[cfg(desktop)]
            updates::update_support,
            #[cfg(desktop)]
            updates::update_check,
            #[cfg(desktop)]
            updates::update_install,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the desktop app")
        .run(|app, event| {
            // Shut the local database down cleanly, whatever closed the app.
            if let tauri::RunEvent::Exit = event {
                app.state::<backend::Backend>().stop();
            }
        });
}

/// Treats SIGTERM, SIGINT and SIGHUP (`kill`, logging out, Ctrl+C or closing the terminal it was
/// started from) like closing the window. Without this the process dies at once and leaves the local
/// server's PostgreSQL running; this way the exit handler above stops it.
#[cfg(unix)]
fn exit_on_signal(app: tauri::AppHandle) {
    use tokio::signal::unix::{SignalKind, signal};

    tauri::async_runtime::spawn(async move {
        let (Ok(mut terminate), Ok(mut interrupt), Ok(mut hangup)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::interrupt()),
            signal(SignalKind::hangup()),
        ) else {
            log::error!("can't listen for shutdown signals");
            return;
        };
        tokio::select! {
            _ = terminate.recv() => {}
            _ = interrupt.recv() => {}
            _ = hangup.recv() => {}
        }
        app.exit(0);
    });
}
