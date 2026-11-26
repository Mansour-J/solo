//! The counts Business health reads. Row-level security limits every query to the current business.

use chrono::NaiveDate;
use sqlx::PgConnection;

/// (invoices say how to pay, payment reminders are on).
pub async fn invoicing(conn: &mut PgConnection) -> Result<(bool, bool), sqlx::Error> {
    sqlx::query_as(
        "SELECT (btrim(bank_account_number) <> '' OR btrim(payment_instructions) <> ''), reminders_enabled
         FROM business_settings",
    )
    .fetch_one(conn)
    .await
}

/// (customers who owe money on an issued invoice, of whom without an email address).
pub async fn customers_owing(conn: &mut PgConnection) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT count(DISTINCT c.id),
                count(DISTINCT c.id) FILTER (WHERE c.email IS NULL OR btrim(c.email::text) = '')
         FROM invoice_balances b
         JOIN customers c ON c.id = b.customer_id
         WHERE b.status = 'issued' AND b.paid < b.total",
    )
    .fetch_one(conn)
    .await
}

/// (expenses dated in `[from, to]` without a receipt, of which claim GST on more than $50).
/// Matches the expenses list's "Missing receipt" filter for the same dates.
pub async fn expenses_without_receipt(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE e.gst_amount > 0 AND e.gross_amount > 5000)
         FROM expenses e
         WHERE e.voided_at IS NULL AND e.expense_date BETWEEN $1 AND $2
           AND NOT EXISTS (SELECT 1 FROM expense_attachments a WHERE a.expense_id = e.id)",
    )
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
}
