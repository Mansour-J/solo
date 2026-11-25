//! The app's books, on this computer (ADR 0014): the local server (`solo-local`: PostgreSQL and the API
//! in this process), started in the background when the app opens and stopped when it closes.
//!
//! The server answers only requests carrying its per-start key, which only this app's window is given
//! (`backend_status`), so other programs and web pages can't read or change the books.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use solo_local::{LocalServer, Options};
use tauri::{AppHandle, Manager, State};

/// What the webview needs to know to talk to the server.
#[derive(Debug, Clone, Serialize)]
pub struct BackendStatus {
    /// `starting` (the local server is coming up), `ready` or `failed`.
    pub state: &'static str,
    /// The API's base URL once ready.
    pub url: Option<String>,
    /// The key to send in `x-api-key` with every request.
    pub key: Option<String>,
    pub error: Option<String>,
}

/// Shared between the commands, the start-up task and the exit handler.
pub struct Backend {
    status: Mutex<BackendStatus>,
    server: Mutex<Option<LocalServer>>,
}

impl Default for Backend {
    fn default() -> Self {
        Self {
            status: Mutex::new(BackendStatus {
                state: "starting",
                url: None,
                key: None,
                error: None,
            }),
            server: Mutex::new(None),
        }
    }
}

impl Backend {
    /// Starts the local server in the background.
    pub fn start(app: &AppHandle, data_dir: PathBuf) {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let backend = handle.state::<Backend>();
            let options = Options {
                data_dir: data_dir.join("local"),
                cors_allowed_origins: vec![
                    "tauri://localhost".into(),
                    "http://tauri.localhost".into(),
                    "http://localhost:5173".into(),
                ],
                require_key: true,
            };
            let result = LocalServer::start(options).await;
            let mut status = backend.status.lock().expect("status lock");
            match result {
                Ok(server) => {
                    status.state = "ready";
                    status.url = Some(server.url().to_owned());
                    status.key = server.key().map(str::to_owned);
                    *backend.server.lock().expect("server lock") = Some(server);
                }
                Err(error) => {
                    log::error!("local server failed to start: {error:#}");
                    status.state = "failed";
                    status.error = Some(format!("{error:#}"));
                }
            }
        });
    }

    /// Stops the local server (and its database) before the app exits or restarts.
    ///
    /// Callable from anywhere: the exit handler, commands, and async code such as installing an update.
    /// The wait happens on a thread of its own, because blocking on a future from a thread that drives
    /// async tasks panics.
    pub fn stop(&self) {
        let Some(server) = self.server.lock().expect("server lock").take() else {
            return;
        };
        match std::thread::spawn(move || tauri::async_runtime::block_on(server.stop())).join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => log::error!("stopping the local server: {error:#}"),
            Err(_) => log::error!("stopping the local server panicked"),
        }
    }
}

#[tauri::command]
pub fn backend_status(backend: State<'_, Backend>) -> BackendStatus {
    backend.status.lock().expect("status lock").clone()
}

/// Stops the local server and restarts the app (after the server failed to start).
#[tauri::command]
pub fn restart_app(app: AppHandle) {
    app.state::<Backend>().stop();
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_starts_out_starting_with_no_address_or_key() {
        let status = Backend::default().status.into_inner().expect("status");
        assert_eq!(status.state, "starting");
        assert!(status.url.is_none() && status.key.is_none() && status.error.is_none());
    }
}
