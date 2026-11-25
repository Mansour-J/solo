//! The accountant pack (SL-511): one zip for a financial year with every report (PDF and Excel), the
//! general journal, the mileage log and the expense receipts.

use std::io::{Cursor, Write};

use chrono::NaiveDate;
use sqlx::PgConnection;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

use crate::context::BusinessContext;
use crate::domain::business::{ReportingBasis, financial_year};
use crate::error::{AppError, AppResult};
use crate::services::export::{CsvWriter, number_cell, text_cell};
use crate::services::invoices::load_business;
use crate::services::{mileage, report_export, reports};
use crate::state::AppState;

#[derive(sqlx::FromRow)]
struct JournalLine {
    entry_date: NaiveDate,
    description: String,
    source_type: String,
    code: String,
    account: String,
    debit: i64,
    credit: i64,
}

async fn journal_lines(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    basis: &str,
) -> Result<Vec<JournalLine>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.entry_date, e.description, e.source_type, a.code, a.name AS account, l.debit, l.credit
         FROM journal_entry_lines l
         JOIN journal_entries e ON e.id = l.entry_id
         JOIN financial_accounts a ON a.id = l.account_id
         WHERE e.entry_date BETWEEN $1 AND $2 AND e.basis IN ('both', $3)
         ORDER BY e.entry_date, e.created_at, e.id, l.debit DESC, a.code",
    )
    .bind(from)
    .bind(to)
    .bind(basis)
    .fetch_all(conn)
    .await
}

#[derive(sqlx::FromRow)]
struct Receipt {
    id: Uuid,
    expense_date: NaiveDate,
    supplier: String,
    filename: String,
    object_key: String,
}

async fn receipts(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<Receipt>, sqlx::Error> {
    sqlx::query_as(
        "SELECT a.id, e.expense_date, e.supplier, a.filename, a.object_key
         FROM expense_attachments a JOIN expenses e ON e.id = a.expense_id
         WHERE e.voided_at IS NULL AND e.expense_date BETWEEN $1 AND $2
         ORDER BY e.expense_date, e.supplier, a.created_at",
    )
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}

/// A file name safe on every system: no path separators or reserved characters, at most 80 characters.
fn safe(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"\/:*?"<>|"#.contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_owned();
    let short: String = trimmed.chars().take(80).collect();
    if short.is_empty() {
        "file".into()
    } else {
        short
    }
}

/// The pack for the financial year ending `year_end`, on `basis` (the business's when none), and its
/// file name.
pub async fn build(
    state: &AppState,
    ctx: &BusinessContext,
    year_end: NaiveDate,
    basis: Option<ReportingBasis>,
) -> AppResult<(String, Vec<u8>)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (from, to) = financial_year(
        year_end,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    if to != year_end {
        return Err(AppError::validation(
            "year_end",
            "Choose the last day of a financial year.",
        ));
    }
    let basis = basis.unwrap_or(business.reporting_basis);
    let basis_name = match basis {
        ReportingBasis::Accrual => "accrual",
        ReportingBasis::Cash => "cash",
    };
    let lines = journal_lines(&mut tx, from, to, basis_name).await?;
    let receipts = receipts(&mut tx, from, to).await?;
    tx.commit().await?;

    let all = reports::build_all(state, ctx, Some(from), Some(to), Some(basis)).await?;
    let title = format!("{} {} to {}", business.name, from, to);
    let brand = &state.config.brand;
    let pdf = report_export::to_pdf_many(&all, &title, &brand.primary_color, &brand.name).await?;
    let xlsx = report_export::to_xlsx_many(&all)?;

    let mut journal = CsvWriter::new();
    journal.row(
        [
            "Date",
            "Entry",
            "Record",
            "Account code",
            "Account",
            "Debit",
            "Credit",
        ]
        .map(text_cell)
        .to_vec(),
    );
    let currency = crate::domain::money::Currency::from_code(&business.currency)
        .ok_or_else(|| AppError::internal(anyhow::anyhow!("unknown currency")))?;
    for l in &lines {
        journal.row(vec![
            text_cell(&l.entry_date.format("%Y-%m-%d").to_string()),
            text_cell(&l.description),
            text_cell(&l.source_type),
            text_cell(&l.code),
            text_cell(&l.account),
            number_cell(currency.to_decimal(l.debit)),
            number_cell(currency.to_decimal(l.credit)),
        ]);
    }
    let (_, mileage_csv) = mileage::export_csv(state, ctx, Some(from), Some(to), None).await?;

    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let mut add = |name: &str, bytes: &[u8]| -> AppResult<()> {
        zip.start_file(name, options).map_err(AppError::internal)?;
        zip.write_all(bytes).map_err(AppError::internal)?;
        Ok(())
    };
    add(&format!("Reports {from} to {to}.pdf"), &pdf)?;
    add(&format!("Reports {from} to {to}.xlsx"), &xlsx)?;
    add("General journal.csv", &journal.into_bytes())?;
    add("Mileage log.csv", &mileage_csv)?;
    let mut missing = Vec::new();
    for (i, r) in receipts.iter().enumerate() {
        match state.storage.get(&r.object_key).await? {
            Some(bytes) => add(
                &format!(
                    "Receipts/{} {} {:03} {}",
                    r.expense_date,
                    safe(&r.supplier),
                    i + 1,
                    safe(&r.filename)
                ),
                &bytes,
            )?,
            None => missing.push(format!("{} ({})", r.filename, r.id)),
        }
    }
    let mut readme = format!(
        "{}\nAccountant pack for the financial year {from} to {to}, on the {basis_name} basis.\n\n\
         Reports … .pdf / .xlsx   every report for the year (profit and loss, balance sheet, changes in equity,\n\
                                  depreciation schedule, summaries, GST, mileage, allocations)\n\
         General journal.csv      every journal line in the year, the source of every figure\n\
         Mileage log.csv          every trip in the year\n\
         Receipts/                the receipts attached to the year's expenses ({} files)\n",
        business.name,
        receipts.len() - missing.len()
    );
    if !missing.is_empty() {
        readme.push_str(&format!(
            "\nThese receipt files couldn't be read and are missing: {}\n",
            missing.join(", ")
        ));
    }
    add("README.txt", readme.as_bytes())?;
    let bytes = zip.finish().map_err(AppError::internal)?.into_inner();
    Ok((
        format!("{} accountant pack {to}.zip", safe(&business.name)),
        bytes,
    ))
}
