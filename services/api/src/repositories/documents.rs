//! Line items shared by invoices and quotes, and document numbering.

use rust_decimal::Decimal;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::documents::{CalculatedLine, document_number};
use crate::domain::tax::TaxTreatment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    Invoice,
    Quote,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct ItemRow {
    pub id: Uuid,
    pub position: i16,
    pub description: String,
    #[schema(value_type = String)]
    pub quantity: Decimal,
    pub unit_price: i64,
    #[schema(value_type = String)]
    pub discount_percent: Decimal,
    pub tax_treatment: TaxTreatment,
    #[schema(value_type = String)]
    pub tax_rate: Decimal,
    pub discount_amount: i64,
    pub net_amount: i64,
    pub tax_amount: i64,
    pub total_amount: i64,
}

macro_rules! item_columns {
    () => {
        "id, position, description, quantity, unit_price, discount_percent, tax_treatment, tax_rate, discount_amount, net_amount, tax_amount, total_amount"
    };
}

macro_rules! insert_items {
    ($table:literal, $fk:literal) => {
        concat!(
            "INSERT INTO ",
            $table,
            " (business_id, ",
            $fk,
            ", position, description, quantity, unit_price, discount_percent, tax_treatment, tax_rate,
               discount_amount, net_amount, tax_amount, total_amount)
             SELECT $1, $2, p, d, q, u, dp, t, r, da, n, ta, tt
             FROM unnest($3::smallint[], $4::text[], $5::numeric[], $6::bigint[], $7::numeric[], $8::text[], $9::numeric[],
                         $10::bigint[], $11::bigint[], $12::bigint[], $13::bigint[]) AS x (p, d, q, u, dp, t, r, da, n, ta, tt)"
        )
    };
}

/// Replaces all lines of a document with freshly calculated ones.
pub async fn replace_items(
    conn: &mut PgConnection,
    kind: DocKind,
    business_id: Uuid,
    document_id: Uuid,
    lines: &[CalculatedLine],
) -> Result<(), sqlx::Error> {
    let (delete, insert) = match kind {
        DocKind::Invoice => (
            "DELETE FROM invoice_items WHERE invoice_id = $1",
            insert_items!("invoice_items", "invoice_id"),
        ),
        DocKind::Quote => (
            "DELETE FROM quote_items WHERE quote_id = $1",
            insert_items!("quote_items", "quote_id"),
        ),
    };
    sqlx::query(delete)
        .bind(document_id)
        .execute(&mut *conn)
        .await?;
    let positions: Vec<i16> = (0..lines.len()).map(|i| i as i16).collect();
    let treatments: Vec<String> = lines
        .iter()
        .map(|l| {
            serde_json::to_value(l.tax_treatment)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        })
        .collect();
    sqlx::query(insert)
        .bind(business_id)
        .bind(document_id)
        .bind(&positions)
        .bind(
            lines
                .iter()
                .map(|l| l.description.clone())
                .collect::<Vec<_>>(),
        )
        .bind(lines.iter().map(|l| l.quantity).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.unit_price).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.discount_percent).collect::<Vec<_>>())
        .bind(&treatments)
        .bind(lines.iter().map(|l| l.tax_rate).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.discount_amount).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.net_amount).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.tax_amount).collect::<Vec<_>>())
        .bind(lines.iter().map(|l| l.total_amount).collect::<Vec<_>>())
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn items(
    conn: &mut PgConnection,
    kind: DocKind,
    document_id: Uuid,
) -> Result<Vec<ItemRow>, sqlx::Error> {
    let sql = match kind {
        DocKind::Invoice => concat!(
            "SELECT ",
            item_columns!(),
            " FROM invoice_items WHERE invoice_id = $1 ORDER BY position"
        ),
        DocKind::Quote => concat!(
            "SELECT ",
            item_columns!(),
            " FROM quote_items WHERE quote_id = $1 ORDER BY position"
        ),
    };
    sqlx::query_as(sql).bind(document_id).fetch_all(conn).await
}

/// Takes the next document number from the business settings (locking the row so concurrent
/// requests can't take the same number), skipping numbers already used.
pub async fn next_number(
    conn: &mut PgConnection,
    kind: DocKind,
    business_id: Uuid,
) -> Result<String, sqlx::Error> {
    let (select, update, exists) = match kind {
        DocKind::Invoice => (
            "SELECT invoice_prefix, next_invoice_number, number_padding FROM business_settings WHERE business_id = $1 FOR UPDATE",
            "UPDATE business_settings SET next_invoice_number = $2 WHERE business_id = $1",
            "SELECT EXISTS (SELECT 1 FROM invoices WHERE business_id = $1 AND number = $2)",
        ),
        DocKind::Quote => (
            "SELECT quote_prefix, next_quote_number, number_padding FROM business_settings WHERE business_id = $1 FOR UPDATE",
            "UPDATE business_settings SET next_quote_number = $2 WHERE business_id = $1",
            "SELECT EXISTS (SELECT 1 FROM quotes WHERE business_id = $1 AND number = $2)",
        ),
    };
    let (prefix, mut next, padding): (String, i32, i16) = sqlx::query_as(select)
        .bind(business_id)
        .fetch_one(&mut *conn)
        .await?;
    let number = loop {
        let candidate = document_number(&prefix, next, padding);
        next += 1;
        let taken: bool = sqlx::query_scalar(exists)
            .bind(business_id)
            .bind(&candidate)
            .fetch_one(&mut *conn)
            .await?;
        if !taken {
            break candidate;
        }
    };
    sqlx::query(update)
        .bind(business_id)
        .bind(next)
        .execute(conn)
        .await?;
    Ok(number)
}

/// Whether a document number is already used (settings validation).
pub async fn number_taken(
    conn: &mut PgConnection,
    kind: DocKind,
    business_id: Uuid,
    number: &str,
) -> Result<bool, sqlx::Error> {
    let sql = match kind {
        DocKind::Invoice => {
            "SELECT EXISTS (SELECT 1 FROM invoices WHERE business_id = $1 AND number = $2)"
        }
        DocKind::Quote => {
            "SELECT EXISTS (SELECT 1 FROM quotes WHERE business_id = $1 AND number = $2)"
        }
    };
    sqlx::query_scalar(sql)
        .bind(business_id)
        .bind(number)
        .fetch_one(conn)
        .await
}
