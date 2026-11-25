//! Keeps the journal in step with the records it describes.
//!
//! A record (invoice, payment, expense, income) states the entries it *should* have. `reconcile`
//! compares that with the entries in effect: identical entries stay; anything else is reversed (a
//! mirror entry on the original date) and the new version posted. Edits and deletions therefore never
//! rewrite history: the journal shows the original, its reversal and the correction.

use std::collections::HashMap;

use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::ledger::{AccountRef, EntrySpec};
use crate::error::{AppError, AppResult};
use crate::repositories::ledger::{self, ActiveEntry, LineRow, NewEntry};

pub const SOURCE_INVOICE: &str = "invoice";
pub const SOURCE_PAYMENT: &str = "invoice_payment";
pub const SOURCE_EXPENSE: &str = "expense";
pub const SOURCE_INCOME: &str = "income";
pub const SOURCE_MILEAGE: &str = "mileage_trip";
pub const SOURCE_CASH_MOVEMENT: &str = "cash_movement";
pub const SOURCE_FIXED_ASSET: &str = "fixed_asset";
pub const SOURCE_DEPRECIATION: &str = "depreciation";
pub const SOURCE_OPENING: &str = "opening_balance";
pub const SOURCE_HOME_OFFICE: &str = "home_office";
pub const SOURCE_ASSET_USE_REVIEW: &str = "asset_use_review";

/// An entry a record should have.
pub struct Desired {
    pub source_type: &'static str,
    pub source_id: Uuid,
    pub spec: EntrySpec,
}

pub struct Context {
    pub business_id: Uuid,
    pub currency: String,
    pub user_id: Option<Uuid>,
}

fn resolve(spec: &EntrySpec, accounts: &HashMap<String, Uuid>) -> AppResult<Vec<LineRow>> {
    let mut lines = spec
        .lines
        .iter()
        .map(|l| {
            let account_id = match l.account {
                AccountRef::Id(id) => id,
                AccountRef::System(system) => *accounts.get(system.key()).ok_or_else(|| {
                    AppError::internal(anyhow::anyhow!(
                        "account {} missing from the chart of accounts",
                        system.key()
                    ))
                })?,
            };
            Ok(LineRow {
                account_id,
                debit: l.debit,
                credit: l.credit,
                customer_id: l.customer_id,
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    lines.sort();
    Ok(lines)
}

fn same(entry: &ActiveEntry, desired: &Desired, lines: &[LineRow]) -> bool {
    let mut existing = entry.lines.clone();
    existing.sort();
    entry.source_type == desired.source_type
        && entry.source_id == desired.source_id
        && entry.basis == desired.spec.basis.as_str()
        && entry.entry_date == desired.spec.date
        && existing == lines
}

/// Makes the journal for `sources` match `desired`, reversing and posting only what changed.
pub async fn reconcile(
    conn: &mut PgConnection,
    ctx: &Context,
    sources: &[(&'static str, Uuid)],
    desired: Vec<Desired>,
) -> AppResult<()> {
    for d in &desired {
        if !d.spec.is_valid() {
            return Err(AppError::internal(anyhow::anyhow!(
                "unbalanced journal entry for {} {}",
                d.source_type,
                d.source_id
            )));
        }
    }
    let accounts = ledger::system_accounts(conn).await?;
    let mut active = ledger::active_entries(conn, sources).await?;
    let mut to_post = Vec::new();
    for d in desired {
        let lines = resolve(&d.spec, &accounts)?;
        if let Some(index) = active.iter().position(|e| same(e, &d, &lines)) {
            active.remove(index);
        } else {
            to_post.push((d, lines));
        }
    }
    // Whatever is still active no longer matches the record: reverse it.
    for stale in active {
        let mirrored: Vec<LineRow> = stale
            .lines
            .iter()
            .map(|l| LineRow {
                account_id: l.account_id,
                debit: l.credit,
                credit: l.debit,
                customer_id: l.customer_id,
            })
            .collect();
        let description = format!("Reversal: {}", stale.description);
        ledger::insert_entry(
            conn,
            NewEntry {
                business_id: ctx.business_id,
                entry_date: stale.entry_date,
                basis: &stale.basis,
                source_type: &stale.source_type,
                source_id: stale.source_id,
                description: &description,
                currency: &ctx.currency,
                reverses_entry_id: Some(stale.id),
                created_by: ctx.user_id,
                lines: &mirrored,
            },
        )
        .await?;
    }
    for (d, lines) in to_post {
        ledger::insert_entry(
            conn,
            NewEntry {
                business_id: ctx.business_id,
                entry_date: d.spec.date,
                basis: d.spec.basis.as_str(),
                source_type: d.source_type,
                source_id: d.source_id,
                description: &d.spec.description,
                currency: &ctx.currency,
                reverses_entry_id: None,
                created_by: ctx.user_id,
                lines: &lines,
            },
        )
        .await?;
    }
    Ok(())
}
