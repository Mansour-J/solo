//! Queries insights need beyond `repositories::reports`: payment times, receivables at past dates,
//! money in and out, and the notices a user has dismissed. Row-level security limits every query to
//! the current business.

use chrono::NaiveDate;
use sqlx::PgConnection;
use uuid::Uuid;

/// Invoices paid in full whose last payment fell within `[from, to]`: (issued, fully paid on).
pub async fn payment_times(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<(NaiveDate, NaiveDate)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT b.issue_date, max(p.payment_date) AS paid_on
         FROM invoice_balances b
         JOIN invoice_payments p ON p.invoice_id = b.invoice_id AND p.voided_at IS NULL
         WHERE b.status = 'issued' AND b.paid >= b.total
         GROUP BY b.invoice_id, b.issue_date
         HAVING max(p.payment_date) BETWEEN $1 AND $2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}

/// What customers owed at the end of `date`: issued invoices dated by then, less payments dated by
/// then (payments removed since don't count).
pub async fn outstanding_at(conn: &mut PgConnection, date: NaiveDate) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(greatest(i.total - coalesce((
                    SELECT sum(p.amount) FROM invoice_payments p
                    WHERE p.invoice_id = i.id AND p.voided_at IS NULL AND p.payment_date <= $1), 0), 0)), 0)::bigint
         FROM invoices i
         WHERE i.status = 'issued' AND i.issue_date <= $1",
    )
    .bind(date)
    .fetch_one(conn)
    .await
}

/// Money recorded as received (invoice payments and income, including GST) and as spent from
/// business funds (expenses not paid personally, including GST) within `[from, to]`.
pub async fn money_in_out(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "SELECT
           (SELECT coalesce(sum(amount), 0) FROM invoice_payments WHERE voided_at IS NULL AND payment_date BETWEEN $1 AND $2)::bigint
         + (SELECT coalesce(sum(gross_amount), 0) FROM income_records WHERE voided_at IS NULL AND income_date BETWEEN $1 AND $2)::bigint,
           (SELECT coalesce(sum(gross_amount), 0) FROM expenses
             WHERE voided_at IS NULL AND payment_method <> 'personal_funds' AND expense_date BETWEEN $1 AND $2)::bigint",
    )
    .bind(from)
    .bind(to)
    .fetch_one(conn)
    .await
}

/// Keys of the notices this user has dismissed in the current business.
pub async fn dismissed(conn: &mut PgConnection, user_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT notice_key FROM insight_dismissals WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(conn)
        .await
}

pub async fn dismiss(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    key: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO insight_dismissals (business_id, user_id, notice_key) VALUES ($1, $2, $3)
         ON CONFLICT (business_id, user_id, notice_key) DO NOTHING",
    )
    .bind(business_id)
    .bind(user_id)
    .bind(key)
    .execute(conn)
    .await
    .map(|_| ())
}

/// Brings back every notice this user dismissed in the current business.
pub async fn restore_all(conn: &mut PgConnection, user_id: Uuid) -> Result<u64, sqlx::Error> {
    sqlx::query("DELETE FROM insight_dismissals WHERE user_id = $1")
        .bind(user_id)
        .execute(conn)
        .await
        .map(|r| r.rows_affected())
}
