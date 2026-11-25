//! Database access. Business data is only read or written inside a transaction that carries the
//! request context (`solo.user_id`, `solo.business_id`), which PostgreSQL row-level security checks on
//! every row. The service layer has already verified membership; RLS is the second line of defence.

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::config::DatabaseConfig;

pub type Tx = Transaction<'static, Postgres>;

#[derive(Clone, Debug)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn connect(config: &DatabaseConfig) -> anyhow::Result<Self> {
        let options: PgConnectOptions = config.url.parse()?;
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(config.acquire_timeout)
            .idle_timeout(Duration::from_secs(600))
            // Unqualified names resolve to the application schema.
            .after_connect(|conn, _| {
                Box::pin(async move {
                    sqlx::query("SET search_path = solo, public")
                        .execute(conn)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await?;
        Ok(Self { pool })
    }

    /// A pool that connects on first use (tools that never touch the database, e.g. OpenAPI export).
    pub fn connect_lazy(config: &DatabaseConfig) -> anyhow::Result<Self> {
        let options: PgConnectOptions = config.url.parse()?;
        Ok(Self {
            pool: PgPoolOptions::new()
                .max_connections(1)
                .connect_lazy_with(options),
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// A transaction with no request context: for authentication tables only (users, sessions, tokens).
    pub async fn begin(&self) -> Result<Tx, sqlx::Error> {
        self.pool.begin().await
    }

    /// A transaction acting as `user_id`, with no business selected (listing businesses, account data).
    pub async fn begin_user(&self, user_id: Uuid) -> Result<Tx, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('solo.user_id', $1, true)")
            .bind(user_id.to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    /// A transaction acting as `user_id` inside `business_id`. Callers must have verified membership.
    pub async fn begin_business(
        &self,
        user_id: Uuid,
        business_id: Uuid,
    ) -> Result<Tx, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "SELECT set_config('solo.user_id', $1, true), set_config('solo.business_id', $2, true)",
        )
        .bind(user_id.to_string())
        .bind(business_id.to_string())
        .execute(&mut *tx)
        .await?;
        Ok(tx)
    }

    pub async fn ping(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
}
