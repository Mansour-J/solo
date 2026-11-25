//! Exports in the CSV formats Xero imports (SL-704): sales invoices and bills (expenses). Amounts are
//! GST-exclusive (choose "Tax exclusive" when importing), one line per invoice line or expense.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use sqlx::PgConnection;
use utoipa::{IntoParams, ToSchema};

use crate::context::BusinessContext;
use crate::domain::money::Currency;
use crate::error::{AppError, AppResult};
use crate::services::export::{CsvWriter, number_cell, text_cell};
use crate::services::invoices::load_business;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XeroExportKind {
    /// Issued invoices (Xero's sales invoice import).
    Sales,
    /// Expenses, the business share (Xero's bill import).
    Bills,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct XeroExportQuery {
    #[param(inline)]
    pub kind: XeroExportKind,
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(sqlx::FromRow)]
struct SalesLine {
    customer: String,
    email: Option<String>,
    number: Option<String>,
    reference: String,
    issue_date: NaiveDate,
    due_date: NaiveDate,
    description: String,
    quantity: Decimal,
    net_amount: i64,
    tax_treatment: String,
}

#[derive(sqlx::FromRow)]
struct BillLine {
    supplier: String,
    expense_date: NaiveDate,
    description: String,
    category: String,
    account_code: String,
    net_amount: i64,
    gst_amount: i64,
    business_percentage: Decimal,
    payment_reference: String,
}

async fn sales(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<SalesLine>, sqlx::Error> {
    sqlx::query_as(
        "SELECT c.name AS customer, c.email, i.number, i.reference, i.issue_date, i.due_date, l.description, l.quantity,
                l.net_amount, l.tax_treatment
         FROM invoice_items l JOIN invoices i ON i.id = l.invoice_id JOIN customers c ON c.id = i.customer_id
         WHERE i.status = 'issued' AND i.issue_date BETWEEN $1 AND $2
         ORDER BY i.issue_date, i.number, l.position",
    )
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}

async fn bills(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<BillLine>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.supplier, e.expense_date, e.description, c.name AS category, a.code AS account_code, e.net_amount,
                e.gst_amount, e.business_percentage, e.payment_reference
         FROM expenses e JOIN expense_categories c ON c.id = e.category_id JOIN financial_accounts a ON a.id = c.account_id
         WHERE e.voided_at IS NULL AND e.business_percentage > 0 AND e.expense_date BETWEEN $1 AND $2
         ORDER BY e.expense_date, e.created_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}

fn sales_tax(treatment: &str) -> &'static str {
    match treatment {
        "standard" => "15% GST on Income",
        "zero_rated" => "Zero Rated Income",
        "exempt" => "Exempt Income",
        _ => "No GST",
    }
}

fn share(amount: i64, percentage: Decimal) -> Decimal {
    (Decimal::from(amount) * percentage / Decimal::ONE_HUNDRED).round()
}

/// The CSV and its file name.
pub async fn export(
    state: &AppState,
    ctx: &BusinessContext,
    q: XeroExportQuery,
) -> AppResult<(String, Vec<u8>)> {
    if q.to < q.from {
        return Err(AppError::validation(
            "to",
            "The end date can’t be before the start date.",
        ));
    }
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let currency = Currency::from_code(&business.currency)
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("unknown currency")))?;
    let money = |minor: Decimal| {
        number_cell(minor / Decimal::from(currency.to_minor(Decimal::ONE).unwrap_or(100)))
    };
    let date = |d: NaiveDate| text_cell(&d.format("%d/%m/%Y").to_string());
    let mut csv = CsvWriter::new();
    let name = match q.kind {
        XeroExportKind::Sales => {
            csv.row(
                [
                    "*ContactName",
                    "EmailAddress",
                    "*InvoiceNumber",
                    "Reference",
                    "*InvoiceDate",
                    "*DueDate",
                    "*Description",
                    "*Quantity",
                    "*UnitAmount",
                    "*AccountCode",
                    "*TaxType",
                    "Currency",
                ]
                .map(text_cell)
                .to_vec(),
            );
            for l in sales(&mut tx, q.from, q.to).await? {
                // One unit at the line's amount, so Xero's totals match exactly; the quantity is in the
                // description.
                let description = if l.quantity == Decimal::ONE {
                    l.description.clone()
                } else {
                    format!("{} (quantity {})", l.description, l.quantity.normalize())
                };
                csv.row(vec![
                    text_cell(&l.customer),
                    text_cell(l.email.as_deref().unwrap_or("")),
                    text_cell(l.number.as_deref().unwrap_or("")),
                    text_cell(&l.reference),
                    date(l.issue_date),
                    date(l.due_date),
                    text_cell(&description),
                    number_cell(1),
                    money(Decimal::from(l.net_amount)),
                    text_cell("200"),
                    text_cell(sales_tax(&l.tax_treatment)),
                    text_cell(&business.currency),
                ]);
            }
            "Xero sales invoices"
        }
        XeroExportKind::Bills => {
            csv.row(
                [
                    "*ContactName",
                    "*InvoiceNumber",
                    "*InvoiceDate",
                    "*DueDate",
                    "*Description",
                    "*Quantity",
                    "*UnitAmount",
                    "*AccountCode",
                    "*TaxType",
                    "Currency",
                ]
                .map(text_cell)
                .to_vec(),
            );
            for (i, l) in bills(&mut tx, q.from, q.to).await?.into_iter().enumerate() {
                let description = if l.description.is_empty() {
                    l.category.clone()
                } else {
                    format!("{}: {}", l.category, l.description)
                };
                let reference = if l.payment_reference.is_empty() {
                    format!("{}-{:04}", l.expense_date.format("%Y%m%d"), i + 1)
                } else {
                    l.payment_reference.clone()
                };
                csv.row(vec![
                    text_cell(&l.supplier),
                    text_cell(&reference),
                    date(l.expense_date),
                    date(l.expense_date),
                    text_cell(&description),
                    number_cell(1),
                    money(share(l.net_amount, l.business_percentage)),
                    text_cell(&l.account_code),
                    text_cell(if l.gst_amount > 0 {
                        "15% GST on Expenses"
                    } else {
                        "No GST"
                    }),
                    text_cell(&business.currency),
                ]);
            }
            "Xero bills"
        }
    };
    tx.commit().await?;
    Ok((
        format!("{name} {} to {}.csv", q.from, q.to),
        csv.into_bytes(),
    ))
}
