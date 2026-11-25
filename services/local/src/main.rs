//! Runs the local server outside the desktop app (development, end-to-end tests):
//!
//!   solo-local --data-dir <folder> [--url-file <file>]
//!
//! Prints the API's URL (and writes it to the URL file), then runs until Ctrl+C. It runs without the
//! desktop app's per-start key, for the Vite dev server and Playwright: use it on your own computer only.

use anyhow::{Context, bail};
use solo_local::{LocalServer, Options};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();
    let mut args = std::env::args().skip(1);
    let (mut data_dir, mut url_file) = (None, None);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => data_dir = args.next(),
            "--url-file" => url_file = args.next(),
            other => bail!(
                "unknown argument {other:?}; usage: solo-local --data-dir <folder> [--url-file <file>]"
            ),
        }
    }
    let data_dir = data_dir.context("--data-dir is required")?;
    let server = LocalServer::start(Options {
        data_dir: data_dir.into(),
        cors_allowed_origins: vec![
            "http://localhost:5173".into(),
            "tauri://localhost".into(),
            "http://tauri.localhost".into(),
        ],
        require_key: false,
    })
    .await?;
    println!("{}", server.url());
    if let Some(file) = url_file {
        std::fs::write(&file, server.url()).with_context(|| format!("writing {file}"))?;
    }
    tokio::signal::ctrl_c().await?;
    server.stop().await
}
