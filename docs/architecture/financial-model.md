# Financial model

Correctness rules that every module follows. Implemented in `services/api/src/domain/` (pure,
unit-tested), `services/ledger.rs` (posting) and `repositories/reports.rs` (every total).

## Money

- Integers in the currency's minor unit (`bigint` in SQL, `i64` in Rust, `number` in JSON and
  TypeScript, safe up to ±90 trillion dollars). Currency precision comes from a table
  (`Currency::minor_units`, e.g. NZD 2, JPY 0).
- Quantities, rates and percentages are decimals (`numeric` / `rust_decimal`, strings in JSON).
- Rounding happens only when a decimal becomes money, half away from zero
  (`domain::money::round_to_minor`):
  - invoice lines: `quantity × unit price` rounded per line; GST per line on the rounded line amount;
    document totals are sums of rounded lines, so the PDF always adds up;
  - business-use and deductible portions: `amount × percentage / 100`, rounded once;
  - splitting one amount across several (a payment across invoice lines, a cash-basis allocation):
    largest-remainder method, so parts always sum exactly to the whole.
- Clients never compute authoritative totals; they display what the API returns (live previews call
  the API's calculation endpoint).

## Tax configuration

Rates are versioned data (`tax_rates`, effective dates), never constants in code. A document uses the
rate in force on its date. GST applies only when the business is GST-registered on that date
(`gst_registered_from`). Mileage rates are the user's own data (`mileage_rates` with dated
`mileage_rate_versions`); Solo ships no government rate (see [ADR 0009](../decisions/0009-mileage.md)).

## Ledger

A per-business chart of accounts (`financial_accounts`) and balanced journal entries
(`journal_entries` + `journal_entry_lines`, debits = credits enforced by a deferred constraint).
Reports read only the ledger, so every report adds up the same way ([ADR 0011](../decisions/0011-reports.md));
statements of account and receivables ageing list issued invoices and payments (`invoice_balances`).

| Event | Entry |
|---|---|
| Invoice issued | Dr Accounts receivable (total) · Cr Sales (net) · Cr GST (tax) |
| Payment recorded | Dr Bank · Cr Accounts receivable |
| Expense posted | Dr Expense category (business net) · Dr GST (claimable) · Dr Owner drawings (private part) · Cr Bank. Paid personally: Dr Expense category · Dr GST · Cr Owner funds (business share only; the private part isn't recorded) |
| Other income | Dr Bank · Cr Other income · Cr GST |
| Mileage claim | Dr Mileage claims (expense, `system_key = mileage`) · Cr Owner funds (both bases) |
| Payment with tax withheld | Dr Bank (amount − withheld) · Dr Withholding tax (equity) · Cr Accounts receivable |
| Drawing / funds introduced | Dr Owner drawings · Cr Bank / Dr Bank · Cr Owner funds |
| GST or income tax paid (refunded) | Dr GST or Income tax · Cr Bank (the reverse for a refund) |
| Asset bought | Dr Property, plant and equipment (cost) · Dr GST (claimable) · Cr Bank |
| Depreciation (year end) | Dr Depreciation (business share) · Dr Owner drawings (private share) · Cr Property, plant and equipment |
| Opening balances | Bank, assets brought forward, GST payable and the owner's balances, with Retained earnings as the balancing figure |

**Mileage claims.** A trip's claim is its business distance (distance × business share, rounded to
0.1 km) × the rate per km in force on the trip date, rounded once to cents. The trip stores the rate it
used, so changing a rate never changes recorded claims. A trip with no business share posts nothing.

**Cash and accrual.** Entries carry a basis: `both`, `accrual` or `cash`. Issuing an invoice posts an
accrual-basis entry; each payment posts an accrual entry (bank vs receivable) and a cash-basis entry
recognising that payment's share of sales and GST (see "Partial payments" below). Expenses are paid when
recorded, so they post to both. A report on one basis reads `basis IN ('both', <basis>)`, and states
which basis it used.

**How posting works.** Each record states the entries it should have (`domain::ledger` builders).
`services::ledger::reconcile` compares them with the entries in effect for that record: identical
entries stay, anything else is reversed with a mirror entry on its original date, and the new
version is posted. Adding a second payment therefore posts only that payment; editing an issued
invoice reverses and re-posts only what changed.

**Partial payments on the cash basis** recognise income and GST cumulatively: after payments
totalling *P* of an invoice of total *T*, the recognised GST is `round(GST × P / T)`, and each payment
posts the difference from the previous position. When the invoice is fully paid, exactly its net and
GST have been recognised, whatever the number or order of payments.

**Corrections never rewrite history.** Drafts aren't posted. Changing or cancelling an issued
invoice, payment or expense posts a reversal of the original entry and, when changed, a new entry,
each dated and linked to its source; the audit log records who and why. Reports for a closed period
therefore change only through visible, dated adjustments.

**Annual accounts** ([ADR 0013](../decisions/0013-annual-accounts.md)). Depreciation is the user's rate
× the value at the start of the year (diminishing value) or × cost (straight line), for the months owned in
the year bought (counting the month of purchase), rounded once to cents and never below zero
(`domain::depreciation`). The balance sheet's current year earnings are this financial year's profit;
earlier profit sits in retained earnings. Non-deductible categories are shown as adjustments below net
profit before adjustments. The two-year data set (`tests/fixtures/fy2025.json`, `fy2026.json`) checks the reports
figure by figure against an independent simulation of an accountant's annual report.

**Allocations** (tax set-asides, savings) are planning records linked to receipts, not ledger
entries: the app never moves money ([ADR 0010](../decisions/0010-allocations.md)). Rules apply in priority
order to what is left of each receipt (`domain::allocations`), in whole minor units, so the planned
amounts plus the remainder equal the amount received. The user confirms a plan before it is recorded;
changing a receipt's amount or deleting it reverses confirmed entries (append-only, linked by
`reverses_id`) and, for a change, opens a new plan to review. Balances are sums of entries.
