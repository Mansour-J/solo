//! Shared application state handed to every handler.

use std::sync::Arc;

use crate::config::Config;
use crate::db::Db;
use crate::services::email::{Mailer, MemoryEmailSender};
use crate::services::storage::Storage;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Db,
    pub mailer: Mailer,
    pub storage: Storage,
    /// Present when `EMAIL_PROVIDER=memory` (tests read sent emails from it).
    pub memory_mailbox: Option<MemoryEmailSender>,
}

impl AppState {
    pub async fn from_config(config: Config) -> anyhow::Result<Self> {
        let db = Db::connect(&config.database).await?;
        Self::with_db(config, db)
    }

    pub fn with_db(config: Config, db: Db) -> anyhow::Result<Self> {
        let (mailer, memory_mailbox) = Mailer::from_config(&config.email, &config.brand)?;
        let storage = Storage::from_config(&config.storage)?;
        Ok(Self {
            config: Arc::new(config),
            db,
            mailer,
            storage,
            memory_mailbox,
        })
    }
}
