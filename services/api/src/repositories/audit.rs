//! Append-only audit log. Business events carry a business_id; account events only a user_id.
//! Inserts run inside the caller's transaction, so the log commits (or rolls back) with the change.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

/// Who did it and from where; built from the authenticated request.
#[derive(Debug, Clone, Default)]
pub struct Actor {
    pub user_id: Option<Uuid>,
    pub ip_address: Option<String>,
    pub request_id: Option<String>,
}

pub struct Entry<'a> {
    pub business_id: Option<Uuid>,
    pub action: &'a str,
    pub entity_type: &'a str,
    pub entity_id: Option<Uuid>,
    pub summary: String,
    pub changes: serde_json::Value,
}

pub async fn record(
    conn: &mut PgConnection,
    actor: &Actor,
    entry: Entry<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_logs (business_id, user_id, action, entity_type, entity_id, summary, changes, ip_address, request_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(entry.business_id)
    .bind(actor.user_id)
    .bind(entry.action)
    .bind(entry.entity_type)
    .bind(entry.entity_id)
    .bind(truncate(&entry.summary, 500))
    .bind(entry.changes)
    .bind(actor.ip_address.as_deref())
    .bind(actor.request_id.as_deref())
    .execute(conn)
    .await
    .map(|_| ())
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: Uuid,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<Uuid>,
    pub summary: String,
    pub changes: serde_json::Value,
    pub user_id: Option<Uuid>,
    /// Name of the user who made the change, when they still exist.
    pub actor_name: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct AuditFilter {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

/// The current business's audit log, newest first. Row-level security limits it to that business.
pub async fn list_for_business(
    conn: &mut PgConnection,
    filter: &AuditFilter,
    limit: i64,
    offset: i64,
) -> Result<(Vec<AuditLogRow>, i64), sqlx::Error> {
    let rows = sqlx::query_as(
        "SELECT a.id, a.action, a.entity_type, a.entity_id, a.summary, a.changes, a.user_id,
                nullif(btrim(u.first_name || ' ' || u.last_name), '') AS actor_name, a.created_at
         FROM audit_logs a LEFT JOIN users u ON u.id = a.user_id
         WHERE a.business_id = current_business_id()
           AND ($1::text IS NULL OR a.entity_type = $1)
           AND ($2::uuid IS NULL OR a.entity_id = $2)
         ORDER BY a.created_at DESC, a.id DESC
         LIMIT $3 OFFSET $4",
    )
    .bind(filter.entity_type.as_deref())
    .bind(filter.entity_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *conn)
    .await?;
    let total = sqlx::query_scalar(
        "SELECT count(*) FROM audit_logs WHERE business_id = current_business_id()
           AND ($1::text IS NULL OR entity_type = $1) AND ($2::uuid IS NULL OR entity_id = $2)",
    )
    .bind(filter.entity_type.as_deref())
    .bind(filter.entity_id)
    .fetch_one(conn)
    .await?;
    Ok((rows, total))
}
