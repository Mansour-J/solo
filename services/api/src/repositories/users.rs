//! The owner's profile (the `users` table): a name and preferences. There is one owner per
//! installation, created on first start.

use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub preferences: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "id, first_name, last_name, preferences, created_at, updated_at"
    };
}

/// The owner: the first user ever created (an installation that had accounts keeps the first one).
pub async fn first(conn: &mut PgConnection) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM users ORDER BY created_at, id LIMIT 1"
    ))
    .fetch_optional(conn)
    .await
}

pub async fn insert(conn: &mut PgConnection, id: Uuid) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as(concat!(
        "INSERT INTO users (id) VALUES ($1) RETURNING ",
        columns!()
    ))
    .bind(id)
    .fetch_one(conn)
    .await
}

pub async fn find_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<UserRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", columns!(), " FROM users WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn update_profile(
    conn: &mut PgConnection,
    id: Uuid,
    first_name: &str,
    last_name: &str,
    preferences: &serde_json::Value,
) -> Result<UserRow, sqlx::Error> {
    sqlx::query_as(concat!(
        "UPDATE users SET first_name = $2, last_name = $3, preferences = $4 WHERE id = $1 RETURNING ",
        columns!()
    ))
    .bind(id)
    .bind(first_name)
    .bind(last_name)
    .bind(preferences)
    .fetch_one(conn)
    .await
}
