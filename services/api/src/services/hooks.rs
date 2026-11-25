//! Extension points where business modules plug into the business lifecycle (creation, settings
//! changes, export and deletion), so `businesses.rs` stays unaware of module internals.

use serde_json::{Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::error::AppResult;
use crate::repositories::allocations::{self as allocations_repo, SourceRef};
use crate::repositories::businesses::SettingsRow;
use crate::repositories::{expenses, ledger};
use crate::services::allocations::{self, ReceiptInfo};

/// Seeds module defaults for a new business (runs in the creating transaction): the chart of
/// accounts, the default expense categories and the default allocation destinations.
pub async fn on_business_created(conn: &mut PgConnection, business_id: Uuid) -> AppResult<()> {
    ledger::seed_business(conn, business_id).await?;
    allocations_repo::seed_business(conn, business_id).await?;
    Ok(())
}

/// Money received was recorded (an invoice payment or income): allocations open one for review.
pub async fn on_receipt_recorded(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    receipt: ReceiptInfo,
) -> AppResult<()> {
    allocations::receipt_recorded(conn, ctx, receipt).await
}

/// A receipt's amount or date changed: its allocation follows, reversing a confirmed one if needed.
pub async fn on_receipt_changed(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    receipt: ReceiptInfo,
) -> AppResult<()> {
    allocations::receipt_changed(conn, ctx, receipt).await
}

/// A receipt was deleted: its allocation is reversed or cancelled.
pub async fn on_receipt_removed(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    source: SourceRef,
    reason: &str,
) -> AppResult<()> {
    allocations::receipt_removed(conn, ctx, source, reason).await
}

/// Whether any financial record exists; the currency is locked once one does.
pub async fn has_financial_records(conn: &mut PgConnection, _business_id: Uuid) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM journal_entries) OR EXISTS (SELECT 1 FROM invoices)
             OR EXISTS (SELECT 1 FROM quotes) OR EXISTS (SELECT 1 FROM expenses)
             OR EXISTS (SELECT 1 FROM mileage_trips) OR EXISTS (SELECT 1 FROM cash_movements)
             OR EXISTS (SELECT 1 FROM fixed_assets)
             OR EXISTS (SELECT 1 FROM home_office_claims)",
    )
    .fetch_one(conn)
    .await?;
    Ok(exists)
}

/// Numbering settings are always accepted: issuing skips numbers that are already used, so lowering
/// a counter can never produce a duplicate.
pub async fn check_document_numbering(
    _conn: &mut PgConnection,
    _business_id: Uuid,
    _settings: &SettingsRow,
) -> AppResult<()> {
    Ok(())
}

/// Storage keys of files that belong to the business (deleted with it).
pub async fn stored_files(conn: &mut PgConnection, business_id: Uuid) -> AppResult<Vec<String>> {
    let logo: Option<String> =
        sqlx::query_scalar("SELECT logo_object_key FROM businesses WHERE id = $1")
            .bind(business_id)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    let mut keys: Vec<String> = logo.into_iter().collect();
    keys.extend(expenses::all_attachment_keys(conn).await?);
    Ok(keys)
}

macro_rules! export_table {
    ($table:literal) => {
        concat!(
            "SELECT coalesce(json_agg(t), '[]'::json) FROM (SELECT * FROM ",
            $table,
            " ORDER BY created_at) t"
        )
    };
}

/// Every record of the business for the account export, keyed by table. Row-level security limits
/// each query to the current business.
pub async fn export_records(conn: &mut PgConnection, _business_id: Uuid) -> AppResult<Value> {
    let tables: [(&str, &str); 32] = [
        ("customers", export_table!("customers")),
        ("invoices", export_table!("invoices")),
        ("invoice_items", export_table!("invoice_items")),
        ("invoice_payments", export_table!("invoice_payments")),
        ("quotes", export_table!("quotes")),
        ("quote_items", export_table!("quote_items")),
        ("email_logs", export_table!("email_logs")),
        ("expense_categories", export_table!("expense_categories")),
        ("expenses", export_table!("expenses")),
        ("expense_attachments", export_table!("expense_attachments")),
        ("income_records", export_table!("income_records")),
        ("mileage_rates", export_table!("mileage_rates")),
        (
            "mileage_rate_versions",
            export_table!("mileage_rate_versions"),
        ),
        ("vehicles", export_table!("vehicles")),
        ("mileage_trips", export_table!("mileage_trips")),
        (
            "allocation_destinations",
            export_table!("allocation_destinations"),
        ),
        ("allocation_rules", export_table!("allocation_rules")),
        ("allocations", export_table!("allocations")),
        ("allocation_entries", export_table!("allocation_entries")),
        ("allocation_spending", export_table!("allocation_spending")),
        ("recurring_invoices", export_table!("recurring_invoices")),
        ("cash_movements", export_table!("cash_movements")),
        ("fixed_assets", export_table!("fixed_assets")),
        ("home_office_claims", export_table!("home_office_claims")),
        ("asset_use_reviews", export_table!("asset_use_reviews")),
        ("asset_disposals", export_table!("asset_disposals")),
        (
            "depreciation_entries",
            export_table!("depreciation_entries"),
        ),
        ("opening_balances", export_table!("opening_balances")),
        (
            "insight_dismissals",
            "SELECT coalesce(json_agg(t), '[]'::json) FROM (SELECT * FROM insight_dismissals ORDER BY dismissed_at) t",
        ),
        ("financial_accounts", export_table!("financial_accounts")),
        ("journal_entries", export_table!("journal_entries")),
        (
            "journal_entry_lines",
            "SELECT coalesce(json_agg(t), '[]'::json) FROM (SELECT l.* FROM journal_entry_lines l JOIN journal_entries e ON e.id = l.entry_id ORDER BY e.created_at, l.id) t",
        ),
    ];
    let mut records = Map::new();
    for (name, sql) in tables {
        let value: Value = sqlx::query_scalar(sql).fetch_one(&mut *conn).await?;
        records.insert(name.to_owned(), value);
    }
    Ok(Value::Object(records))
}
