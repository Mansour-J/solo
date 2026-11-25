# 0011 — Reports: one report shape, one export layer, figures from the journal

- **Status:** Accepted
- **Date:** 2026-11-21

## Context

Phase 5 needs nine reports (profit and loss, income and expense summaries, statement of account,
accounts receivable and ageing, financial statement, mileage, allocations, GST) with shared filters,
both bases, and PDF, CSV and Excel exports. Writing a screen, a PDF template and two spreadsheet
layouts per report would multiply the code and let the formats drift apart.

## Decision

- **One shape.** Every report is a `domain::reports::Report`: a title, period and basis, context
  lines, then sections of typed columns (`text`, `money` in minor units, `number`, `count`, `date`),
  rows and an optional totals row, and notes. Services in `services/reports.rs` fill it; nothing
  else formats figures.
- **One endpoint pair.** `GET /reports/{kind}` returns the report as JSON with the shared filters
  (`from`, `to`, `basis`, `customer_id`, `category_id`, `vehicle_id`, `status`); `GET
  /reports/{kind}/export?format=pdf|csv|xlsx` returns the same report as a file. The client renders
  any report with one generic view.
- **One export layer** (`services/report_export.rs`): CSV in UTF-8 with a byte-order mark (formula-like
  text neutralised); Excel through `rust_xlsxwriter` with money as numbers formatted to the currency's
  decimals and dates as dates, so they sum and sort; a branded A4 PDF from one Typst template
  (`assets/templates/report.typ`) with the embedded Inter fonts, landscape only for wide tables.
- **Where figures come from.** Income, expenses, profit and GST are read from journal lines
  (`repositories/reports.rs`, shared with the dashboard), on the chosen basis. Grouped reports run one
  query returning detail rows (by month, account, customer, supplier) and add them up in several
  ways, so the sections of a report always agree. Statements and ageing list issued invoices and
  payments (`invoice_balances`), because that is what a customer is sent and asked to pay.
- **Defaults.** The period defaults to the current financial year and the basis to the business's
  reporting basis. Profit and loss compares with the previous period of the same length (whole
  months with whole months, `domain::reports::previous_period`).

## Consequences

- A new report is a service function that returns a `Report`; its screen, PDF, CSV and Excel come
  for free, and an integration test checks its figures.
- Gross profit equals revenue from sales until cost-of-sales accounts exist (a follow-up), and the
  report says so.
- Report layouts are generic: a report that needs a chart or a custom layout adds it in the client
  around the same data.

## Alternatives considered

- **A template per report and format:** maximum control, but nine reports × three formats to keep in
  step, and the formats would disagree sooner or later.
- **Client-side spreadsheet generation:** keeps the server simpler, but clients must never compute
  authoritative totals, and the desktop app would need the same code.
- **Generating Excel as CSV with a different extension:** Excel opens it, but loses number formats
  and dates and warns about the mismatch.
