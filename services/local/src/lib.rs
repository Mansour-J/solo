//! Solo on the user's own computer (ADR 0014): a private PostgreSQL in the app's data folder, the
//! schema migrations, and the API, all in the desktop app's process. The API listens on a random
//! port on 127.0.0.1 only, and nothing is sent anywhere.
//!
//! Layout of the data folder:
//! - `postgresql/`: the PostgreSQL program, unpacked from the binary on first start
//! - `database/`: the database cluster (the user's records)
//! - `storage/`: receipts and other files
//! - `local.json`: the database owner's password (readable only by this user)
//! - `backups/`: copies of the books kept before a restore, and work folders ([`backups`])

mod backups;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use postgresql_embedded::{PostgreSQL, Settings, VersionReq};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use solo_api::{AppState, Config};
use sqlx::{Connection, PgConnection};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// The bundled PostgreSQL release (`POSTGRESQL_VERSION` in `.cargo/config.toml` must match).
pub const POSTGRES_VERSION: &str = "=18.7.0";
const DATABASE: &str = "solo";
const OWNER: &str = "postgres";

/// Where the local server keeps everything, and which web origins may call it.
#[derive(Debug, Clone)]
pub struct Options {
    pub data_dir: PathBuf,
    /// The desktop webview's origins (`tauri://localhost`, `http://tauri.localhost`).
    pub cors_allowed_origins: Vec<String>,
    /// Require a new random key on every API request ([`LocalServer::key`]), so only the program that
    /// started the server can use it. Off only for development and end-to-end tests.
    pub require_key: bool,
}

struct Paths {
    runtime: PathBuf,
    database: PathBuf,
    storage: PathBuf,
    secrets: PathBuf,
    password_file: PathBuf,
}

impl Paths {
    fn new(root: &Path) -> Self {
        Self {
            runtime: root.join("postgresql"),
            database: root.join("database"),
            storage: root.join("storage"),
            secrets: root.join("local.json"),
            password_file: root.join(".pgpass"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Secrets {
    /// The database owner's password, created on first start.
    owner_password: String,
}

fn random_password() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 32)
}

impl Secrets {
    fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("reading {}", path.display()))?;
            return serde_json::from_str(&text)
                .with_context(|| format!("{} is damaged", path.display()));
        }
        let secrets = Self {
            owner_password: random_password(),
        };
        write_private(path, &serde_json::to_string_pretty(&secrets)?)?;
        Ok(secrets)
    }
}

/// Writes a file only this user can read (on Windows, the user's app-data folder already is).
fn write_private(path: &Path, contents: &str) -> Result<()> {
    std::fs::write(path, contents).with_context(|| format!("writing {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// The running local server. Call [`LocalServer::stop`] before the app exits, so PostgreSQL shuts
/// down cleanly.
pub struct LocalServer {
    url: String,
    key: Option<String>,
    postgres: PostgreSQL,
    shutdown: oneshot::Sender<()>,
    server: JoinHandle<std::io::Result<()>>,
}

impl LocalServer {
    /// Starts PostgreSQL (setting it up on first use), brings the schema up to date and starts the API.
    pub async fn start(options: Options) -> Result<Self> {
        let paths = Paths::new(&options.data_dir);
        for dir in [&options.data_dir, &paths.storage] {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let secrets = Secrets::load_or_create(&paths.secrets)?;
        let mut postgres = PostgreSQL::new(Settings {
            version: VersionReq::parse(POSTGRES_VERSION)?,
            installation_dir: paths.runtime.clone(),
            data_dir: paths.database.clone(),
            password_file: paths.password_file.clone(),
            host: "127.0.0.1".into(),
            port: 0,
            username: OWNER.into(),
            password: secrets.owner_password.clone(),
            temporary: false,
            timeout: Some(Duration::from_secs(60)),
            configuration: HashMap::from([("listen_addresses".into(), "127.0.0.1".into())]),
            ..Settings::default()
        });
        postgres.setup().await.context("setting up the database")?;
        // If the app was killed, its PostgreSQL may still be running on this folder: stop it cleanly
        // (a fast shutdown, so nothing is lost) before starting our own.
        if paths.database.join("postmaster.pid").exists() {
            tracing::warn!("the database was still running from an earlier session; stopping it");
            let _ = postgres.stop().await;
        }
        postgres.start().await.context("starting the database")?;
        let port = postgres.settings().port;
        let bin_dir = postgres.settings().binary_dir();
        let key = options.require_key.then(random_password);
        match Self::prepare(&options, &paths, &secrets, port, &bin_dir, key.as_deref()).await {
            Ok((url, shutdown, server)) => Ok(Self {
                url,
                key,
                postgres,
                shutdown,
                server,
            }),
            Err(error) => {
                let _ = postgres.stop().await;
                Err(error)
            }
        }
    }

    async fn prepare(
        options: &Options,
        paths: &Paths,
        secrets: &Secrets,
        port: u16,
        bin_dir: &Path,
        api_key: Option<&str>,
    ) -> Result<(String, oneshot::Sender<()>, JoinHandle<std::io::Result<()>>)> {
        // The owner creates the database, the API's role and the schema.
        let server_url = format!(
            "postgres://{OWNER}:{}@127.0.0.1:{port}",
            secrets.owner_password
        );
        let mut admin = PgConnection::connect(&format!("{server_url}/postgres"))
            .await
            .context("connecting to the database")?;
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
                .bind(DATABASE)
                .fetch_one(&mut admin)
                .await?;
        if !exists {
            sqlx::raw_sql("CREATE DATABASE solo")
                .execute(&mut admin)
                .await?;
        }
        admin.close().await?;

        let mut owner = PgConnection::connect(&format!("{server_url}/{DATABASE}")).await?;
        // A new password for the API's role on every start: it only ever lives in this process.
        let app_password = random_password();
        sqlx::raw_sql(
            "DO $$ BEGIN
               IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'solo_app') THEN CREATE ROLE solo_app LOGIN; END IF;
             END $$",
        )
        .execute(&mut owner)
        .await?;
        // The password is 32 random letters and digits, so it can't break out of the literal.
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "ALTER ROLE solo_app WITH LOGIN PASSWORD '{app_password}'"
        )))
        .execute(&mut owner)
        .await?;
        let report = solo_api::migrations::migrate(&mut owner, "solo-local")
            .await
            .context("updating the database")?;
        tracing::info!(applied = ?report.applied, version = ?report.current, "local database ready");
        owner.close().await?;

        let database_url =
            format!("postgres://solo_app:{app_password}@127.0.0.1:{port}/{DATABASE}");
        let storage = paths.storage.to_string_lossy().into_owned();
        let origins = options.cors_allowed_origins.join(",");
        let config = Config::from_lookup(|key| {
            match key {
                "APP_ENV" => Some("production"),
                "DATABASE_URL" => Some(database_url.as_str()),
                "DATABASE_MAX_CONNECTIONS" => Some("10"),
                "EMAIL_PROVIDER" => Some("log"),
                "STORAGE_BACKEND" => Some("local"),
                "STORAGE_LOCAL_PATH" => Some(storage.as_str()),
                "API_HOST" => Some("127.0.0.1"),
                "API_KEY" => api_key,
                "CORS_ALLOWED_ORIGINS" => Some(origins.as_str()),
                _ => None,
            }
            .map(str::to_owned)
        })?;
        let mut state = AppState::from_config(config)
            .await
            .context("starting the API")?;
        state.backups = Some(Arc::new(backups::LocalBackups::new(
            bin_dir.to_path_buf(),
            port,
            OWNER,
            &secrets.owner_password,
            DATABASE,
            paths.storage.clone(),
            &options.data_dir,
        )));
        let owner = solo_api::services::users::ensure_owner(&state.db).await?;
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
        let url = format!("http://{}", listener.local_addr()?);
        let (shutdown, signal) = oneshot::channel::<()>();
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                solo_api::app(state, owner).into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = signal.await;
            })
            .await
        });
        tracing::info!(%url, "local server listening");
        Ok((url, shutdown, server))
    }

    /// The API's base URL, e.g. `http://127.0.0.1:53817`.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The key every API request must send in `x-api-key`, when [`Options::require_key`] is on.
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    /// Stops the API, then PostgreSQL.
    pub async fn stop(self) -> Result<()> {
        let _ = self.shutdown.send(());
        let _ = self.server.await;
        self.postgres
            .stop()
            .await
            .context("stopping the database")?;
        tracing::info!("local server stopped");
        Ok(())
    }
}
