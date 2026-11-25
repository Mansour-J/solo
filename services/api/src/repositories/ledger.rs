//! Chart of accounts and journal rows. Entries are only ever inserted (the API has no UPDATE or
//! DELETE privilege on them); corrections insert reversing entries.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

/// Seeds the default chart of accounts and expense categories for a new business.
pub async fn seed_business(conn: &mut PgConnection, business_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT seed_chart_of_accounts($1), seed_expense_categories($1)")
        .bind(business_id)
        .execute(conn)
        .await
        .map(|_| ())
}

/// `system_key → account id` for the current business.
pub async fn system_accounts(
    conn: &mut PgConnection,
) -> Result<HashMap<String, Uuid>, sqlx::Error> {
    let rows: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT system_key, id FROM financial_accounts WHERE business_id = current_business_id() AND system_key IS NOT NULL",
    )
    .fetch_all(conn)
    .await?;
    Ok(rows.into_iter().collect())
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
pub struct LineRow {
    pub account_id: Uuid,
    pub debit: i64,
    pub credit: i64,
    pub customer_id: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct ActiveEntry {
    pub id: Uuid,
    pub source_type: String,
    pub source_id: Uuid,
    pub basis: String,
    pub entry_date: NaiveDate,
    pub description: String,
    pub lines: Vec<LineRow>,
}

#[derive(sqlx::FromRow)]
struct EntryRow {
    id: Uuid,
    source_type: String,
    source_id: Uuid,
    basis: String,
    entry_date: NaiveDate,
    description: String,
}

#[derive(sqlx::FromRow)]
struct EntryLineRow {
    entry_id: Uuid,
    account_id: Uuid,
    debit: i64,
    credit: i64,
    customer_id: Option<Uuid>,
}

/// Entries currently in effect for the given sources: not reversals, and not reversed.
pub async fn active_entries(
    conn: &mut PgConnection,
    sources: &[(&str, Uuid)],
) -> Result<Vec<ActiveEntry>, sqlx::Error> {
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let types: Vec<String> = sources.iter().map(|(t, _)| (*t).to_owned()).collect();
    let ids: Vec<Uuid> = sources.iter().map(|(_, id)| *id).collect();
    let entries: Vec<EntryRow> = sqlx::query_as(
        "SELECT e.id, e.source_type, e.source_id, e.basis, e.entry_date, e.description
         FROM journal_entries e
         JOIN unnest($1::text[], $2::uuid[]) AS s (source_type, source_id)
           ON s.source_type = e.source_type AND s.source_id = e.source_id
         WHERE e.business_id = current_business_id()
           AND e.reverses_entry_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM journal_entries r WHERE r.reverses_entry_id = e.id)
         ORDER BY e.created_at, e.id",
    )
    .bind(&types)
    .bind(&ids)
    .fetch_all(&mut *conn)
    .await?;
    let entry_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();
    let lines: Vec<EntryLineRow> =
        sqlx::query_as("SELECT entry_id, account_id, debit, credit, customer_id FROM journal_entry_lines WHERE entry_id = ANY($1)")
            .bind(&entry_ids)
            .fetch_all(conn)
            .await?;
    let mut by_entry: HashMap<Uuid, Vec<LineRow>> = HashMap::new();
    for l in lines {
        by_entry.entry(l.entry_id).or_default().push(LineRow {
            account_id: l.account_id,
            debit: l.debit,
            credit: l.credit,
            customer_id: l.customer_id,
        });
    }
    Ok(entries
        .into_iter()
        .map(|e| ActiveEntry {
            lines: by_entry.remove(&e.id).unwrap_or_default(),
            id: e.id,
            source_type: e.source_type,
            source_id: e.source_id,
            basis: e.basis,
            entry_date: e.entry_date,
            description: e.description,
        })
        .collect())
}

pub struct NewEntry<'a> {
    pub business_id: Uuid,
    pub entry_date: NaiveDate,
    pub basis: &'a str,
    pub source_type: &'a str,
    pub source_id: Uuid,
    pub description: &'a str,
    pub currency: &'a str,
    pub reverses_entry_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub lines: &'a [LineRow],
}

pub async fn insert_entry(
    conn: &mut PgConnection,
    entry: NewEntry<'_>,
) -> Result<Uuid, sqlx::Error> {
    let id = Uuid::now_v7();
    let description: String = entry.description.chars().take(300).collect();
    sqlx::query(
        "INSERT INTO journal_entries (id, business_id, entry_date, basis, source_type, source_id, description, currency, reverses_entry_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(id)
    .bind(entry.business_id)
    .bind(entry.entry_date)
    .bind(entry.basis)
    .bind(entry.source_type)
    .bind(entry.source_id)
    .bind(&description)
    .bind(entry.currency)
    .bind(entry.reverses_entry_id)
    .bind(entry.created_by)
    .execute(&mut *conn)
    .await?;
    let accounts: Vec<Uuid> = entry.lines.iter().map(|l| l.account_id).collect();
    let debits: Vec<i64> = entry.lines.iter().map(|l| l.debit).collect();
    let credits: Vec<i64> = entry.lines.iter().map(|l| l.credit).collect();
    let customers: Vec<Option<Uuid>> = entry.lines.iter().map(|l| l.customer_id).collect();
    sqlx::query(
        "INSERT INTO journal_entry_lines (business_id, entry_id, account_id, debit, credit, customer_id)
         SELECT $1, $2, a, d, c, cu FROM unnest($3::uuid[], $4::bigint[], $5::bigint[], $6::uuid[]) AS t (a, d, c, cu)",
    )
    .bind(entry.business_id)
    .bind(id)
    .bind(&accounts)
    .bind(&debits)
    .bind(&credits)
    .bind(&customers)
    .execute(conn)
    .await?;
    Ok(id)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct JournalEntryView {
    pub id: Uuid,
    pub entry_date: NaiveDate,
    pub basis: String,
    pub source_type: String,
    pub description: String,
    pub reverses_entry_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    /// Debit total (equals the credit total).
    pub amount: i64,
}

/// Every entry ever posted for a source (including reversals), oldest first: the record's ledger history.
pub async fn history(
    conn: &mut PgConnection,
    sources: &[(&str, Uuid)],
) -> Result<Vec<JournalEntryView>, sqlx::Error> {
    let types: Vec<String> = sources.iter().map(|(t, _)| (*t).to_owned()).collect();
    let ids: Vec<Uuid> = sources.iter().map(|(_, id)| *id).collect();
    sqlx::query_as(
        "SELECT e.id, e.entry_date, e.basis, e.source_type, e.description, e.reverses_entry_id, e.created_at,
                (SELECT coalesce(sum(l.debit), 0)::bigint FROM journal_entry_lines l WHERE l.entry_id = e.id) AS amount
         FROM journal_entries e
         JOIN unnest($1::text[], $2::uuid[]) AS s (source_type, source_id)
           ON s.source_type = e.source_type AND s.source_id = e.source_id
         WHERE e.business_id = current_business_id()
         ORDER BY e.created_at, e.id",
    )
    .bind(&types)
    .bind(&ids)
    .fetch_all(conn)
    .await
}
