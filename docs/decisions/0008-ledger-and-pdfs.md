# 0008 — Dual-basis append-only ledger, and PDFs with Typst

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

Reports must be consistent, support cash and accrual bases, and never silently change history when
an invoice, payment or expense is corrected. Invoices and quotes must be emailed as professional PDFs,
so they are generated on the server.

## Decision

**Ledger.**
- A per-business chart of accounts and a journal of balanced entries (debits = credits, checked by a
  deferred constraint trigger at commit). The API may only insert journal rows.
- Each entry has a basis: `both`, `accrual` or `cash`. Issuing an invoice posts accrual; each payment
  posts accrual (bank vs receivable) and cash (income and GST recognised cumulatively in proportion);
  expenses and income without an invoice post to both. Reports filter on `basis IN ('both', <basis>)`.
- Records declare the entries they should have; `reconcile` reverses whatever no longer matches and
  posts the new version, dated as the original. Edits and deletions are visible as reversals.
- Every report and dashboard figure is a query over journal lines (`repositories/reports.rs`).

**PDFs.** Typst, embedded as a library (`typst-as-lib`), renders a template (`assets/templates/document.typ`)
from JSON with embedded Inter fonts (SIL OFL), on the blocking thread pool.

## Consequences

- One source of truth for every figure; cash and accrual come from the same entries.
- Back-dated corrections change the reports of the period they belong to, but always through
  visible reversing entries and an audit log entry stating who changed what.
- The API binary embeds ~1.6 MB of fonts and compiles Typst (a slower first build).

## Alternatives considered

- **Computing reports from source tables (invoices, payments, expenses) directly:** every report
  re-implements the rules; cash/accrual and corrections diverge between reports.
- **Separate cash and accrual ledgers:** double the entries with no benefit over a basis tag.
- **Headless Chromium (HTML to PDF):** a heavy extra service. **printpdf/krilla:** manual layout of
  tables and page breaks. **Client-side PDFs:** can't attach them to server-sent emails.
