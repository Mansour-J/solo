---
name: ledger-posting
description: Make a Solo financial record post to the double-entry journal correctly — EntrySpec builders in domain::ledger, cash/accrual basis, a SOURCE_* type and its database constraint, system accounts by system_key, services::ledger::reconcile for create/edit/void with reversals, reports that read only journal lines, and the tests that prove the books balance. Use for any record that changes income, expenses, GST, receivables, bank or owner funds, or any new report figure.
---

# Ledger posting

Read `docs/architecture/financial-model.md` and ADR 0008 (`docs/decisions/0008-ledger-and-pdfs.md`) first.

**Rules:**

- Never insert, update or delete journal rows yourself. The API may only insert them, and only `services::ledger::reconcile` does.
- A record declares the entries it _should_ have; `reconcile` makes the journal match.
- Every report and dashboard figure is a query over journal lines.

## 1. Builder (`services/api/src/domain/ledger.rs`)

Write a pure function that returns the `EntrySpec`(s) for the record, next to the existing builders: `invoice_issued`, `invoice_payment`, `expense` (with `split_expense`) and `income`.

```rust
/// A thing (both bases: paid when recorded).
pub fn thing(date: NaiveDate, label: &str, customer: Option<Uuid>, net: i64, gst: i64) -> EntrySpec {
    EntrySpec::new(
        date,
        EntryBasis::Both,
        format!("Thing: {label}"),
        vec![
            LineSpec::debit(Sys(Bank), net + gst),
            LineSpec::credit(Sys(Sales), net).customer(customer),
            LineSpec::credit(Sys(Gst), gst),
        ],
    )
}
```

- **Accounts:**
  - `Sys(SystemAccount::…)`: accounts every business has, resolved by `system_key`.
  - `AccountRef::Id(id)`: per-business accounts, such as each expense category's own account.
- `.customer(id)` on the line that belongs to a customer, so balances per customer come from the journal.
- **Amounts are already rounded minor units.** Compute splits with `domain::money` before building lines (`split_expense` shows how), so the parts sum exactly.
- `EntrySpec::new` drops zero lines (GST on a zero-rated sale). `reconcile` rejects an entry unless `is_valid()`: balanced, at least two lines, each line one positive side.
- Descriptions are limited to 300 characters by the database. Truncate user text such as a payer or supplier name.
- Unit-test the builder in the same file: balanced, the right accounts, rounding and splits.

## 2. Basis

| Record                                                   | Basis                                                                                                    |
| -------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Paid when recorded (expenses, income without an invoice) | `EntryBasis::Both`                                                                                       |
| Invoice issued                                           | `Accrual` (receivable, sales, GST)                                                                       |
| Payment on an invoice                                    | `Accrual` (bank vs receivable) **and** `Cash` (its cumulative share of sales and GST, `invoice_payment`) |

A report on one basis reads `basis IN ('both', <basis>)`. Drafts post nothing.

## 3. Source type

- Add `pub const SOURCE_THING: &str = "thing";` next to `SOURCE_INVOICE`, `SOURCE_PAYMENT`, `SOURCE_EXPENSE` and `SOURCE_INCOME` in `services/api/src/services/ledger.rs`.
- The database only accepts listed types. Add a migration (`add-migration` skill) that drops and re-adds `journal_entries_source_type_check` with the complete list. Copy the current list from the latest migration that defines it: `grep -ln journal_entries_source_type_check database/migrations`.

## 4. System accounts

A new account the application posts to directly needs:

- a `SystemAccount` variant with its `key()` in `domain/ledger.rs`
- a migration that does `CREATE OR REPLACE FUNCTION seed_chart_of_accounts(b uuid)` with the complete list plus the new row (`ON CONFLICT (business_id, system_key) DO NOTHING`), then backfills `SELECT seed_chart_of_accounts(id) FROM businesses;`. Account codes are unique per business: check V5's list and the expense category accounts from `seed_expense_categories` (V8).

If a system key is missing, `reconcile` fails with an internal error rather than posting to the wrong account.

## 5. Posting from the service

Follow `sync_postings` in `services/api/src/services/income.rs` (one source) or `services/api/src/services/invoices.rs` (an invoice and its payments):

```rust
async fn sync_postings(conn: &mut PgConnection, ctx: &BusinessContext, business: &BusinessRow, id: Uuid) -> AppResult<()> {
    let row = repo::find_thing(conn, id).await?.ok_or(AppError::NotFound("Thing"))?;
    let mut desired = Vec::new();
    if row.voided_at.is_none() {
        desired.push(Desired { source_type: SOURCE_THING, source_id: id, spec: ledger::thing(/* from the stored row */) });
    }
    let context = Context { business_id: ctx.business_id, currency: business.currency.clone(), user_id: Some(ctx.user_id()) };
    ledger_service::reconcile(conn, &context, &[(SOURCE_THING, id)], desired).await
}
```

- Call it after every insert, update and void, **in the same transaction** as the change and its `audit::record`.
- Build from the stored row, re-read after writing, so the journal reflects exactly what was saved.
- `sources` lists every source whose entries this record owns (an invoice passes itself and all its payments, including voided ones), so stale entries get reversed.
- `reconcile` keeps identical entries, reverses anything else with a mirror entry on its original date, and posts the new version. Never compute deltas yourself, and never "fix" an old entry: corrections are visible reversals.
- A voided record or a draft desires nothing, so everything it posted is reversed.
- Show a record's postings with `repositories::ledger::history` (the invoice `journal` field, the expense drawer's "How it’s recorded").

## 6. Reports

- Figures come from `services/api/src/repositories/reports.rs` (`profit`, `monthly`, `expenses_by_account`, `gst`, `receivables`): journal lines joined to `financial_accounts`, filtered by date and basis.
- Never sum source tables (invoices, expenses) for a figure. A new figure is a new query there, used by a service such as `services/dashboard.rs`, and states which basis it used.
- Allocations (tax set-asides, savings) are planning records linked to payments, not journal entries: the app never moves money.

## 7. Tests

In `services/api/tests/bookkeeping.rs` (`Books::new`):

- `assert_ledger_balances()` after every scenario
- dashboard totals on both bases: `b.dashboard("cash")` and `b.dashboard("accrual")`
- edit, then check the reversal and the re-post: `corrections_reverse_and_repost_instead_of_rewriting_history`
- void, then check the totals return to zero
- the journal stays append-only for `solo_app`: `the_ledger_is_append_only_for_the_application`.

Then add the event's row to the ledger table in `docs/architecture/financial-model.md`.
