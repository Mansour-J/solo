//! Entry point.
//!
//! * `solo-api` (or `solo-api serve`) — run the HTTP server.
//! * `solo-api openapi` — print the OpenAPI document (used to generate the TypeScript client).
//! * `solo-api healthcheck` — exit 0 if the local server is ready (Docker health check).
//! * `solo-api migrate` — apply the embedded schema migrations to `MIGRATE_DATABASE_URL` (the schema
//!   owner), as Flyway would.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::Context;
use solo_api::{AppState, Config};
use tracing_subscriber::EnvFilter;

fn main() -> anyhow::Result<()> {
    // A local env file is optional; real deployments use the process environment.
    let _ = dotenvy::dotenv();
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    match command.as_str() {
        "serve" => serve(),
        "openapi" => print_openapi(),
        "healthcheck" => healthcheck(),
        "migrate" => migrate(),
        other => anyhow::bail!(
            "unknown command {other:?}; expected serve, openapi, healthcheck or migrate"
        ),
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    if std::env::var("LOG_FORMAT").is_ok_and(|v| v == "json") {
        builder
            .json()
            .flatten_event(true)
            .with_current_span(true)
            .init();
    } else {
        builder.init();
    }
}

#[tokio::main]
async fn serve() -> anyhow::Result<()> {
    init_tracing();
    let config = Config::from_env()?;
    let bind_addr = config.bind_addr;
    tracing::info!(product = %config.brand.name, environment = ?config.environment, "starting API");
    let state = AppState::from_config(config)
        .await
        .context("could not start (is the database reachable?)")?;
    let owner = solo_api::services::users::ensure_owner(&state.db).await?;
    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .with_context(|| format!("cannot listen on {bind_addr}"))?;
    tracing::info!(%bind_addr, "listening");
    axum::serve(
        listener,
        solo_api::app(state, owner).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    tracing::info!("stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}

/// Prints the OpenAPI document without needing a database.
#[tokio::main(flavor = "current_thread")]
async fn print_openapi() -> anyhow::Result<()> {
    let config = Config::from_lookup(|key| match key {
        "DATABASE_URL" => Some("postgres://openapi@localhost/openapi".into()),
        "EMAIL_PROVIDER" => Some("memory".into()),
        "STORAGE_BACKEND" => Some("memory".into()),
        _ => None,
    })?;
    let db = solo_api::db::Db::connect_lazy(&config.database)?;
    let state = AppState::with_db(config, db)?;
    println!("{}", solo_api::api::openapi(&state).to_pretty_json()?);
    Ok(())
}

/// Applies the embedded migrations as the schema owner.
#[tokio::main(flavor = "current_thread")]
async fn migrate() -> anyhow::Result<()> {
    use sqlx::Connection;
    init_tracing();
    let url = std::env::var("MIGRATE_DATABASE_URL")
        .context("MIGRATE_DATABASE_URL (the schema owner's connection string) is required")?;
    let mut conn = sqlx::PgConnection::connect(&url)
        .await
        .context("cannot connect")?;
    let user: String = sqlx::query_scalar("SELECT current_user")
        .fetch_one(&mut conn)
        .await?;
    let report = solo_api::migrations::migrate(&mut conn, &user).await?;
    tracing::info!(applied = ?report.applied, current = ?report.current, "schema up to date");
    Ok(())
}

fn healthcheck() -> anyhow::Result<()> {
    let port = std::env::var("API_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8000);
    let mut stream = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream
        .write_all(b"GET /health/ready HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if response.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        anyhow::bail!("not ready: {}", response.lines().next().unwrap_or_default())
    }
}
