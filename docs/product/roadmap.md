# Roadmap

Each phase ships as working vertical slices: UI, API, migrations, validation, permissions, tests and
docs. A module appears in the navigation only when it works end to end. Items, priorities and status
for each phase are in the [backlog](../backlog/backlog.html) (open it in a browser to filter and group items).

## Phase 0 — Foundation ✅

- Monorepo; Rust/Axum API with layered modules, OpenAPI docs and generated TypeScript client
- PostgreSQL 18 with Flyway migrations, row-level security, schema checks
- React + Mantine client with design system (light/dark, responsive), Tauri 2 desktop shell
- Central branding (`branding/brand.json` + `APP_*`), brand sync for desktop
- Docker Compose stack (database, migrations, API, web, Mailpit), setup/backup/restore/upgrade scripts
  (later replaced by the desktop app's bundled database: ADR 0016)
- GitHub Actions: CI quality gate, tagged releases with installers
- Documentation and ADRs

## Phase 1 — Authentication and onboarding ✅

The accounts, sign-in and sessions below were later removed: the app is for one person with no sign-in
([ADR 0016](../decisions/0016-one-person-no-sign-in.md)). Businesses, onboarding and preferences remain.


- Registration with terms acceptance, email verification and resend
- Sign-in with "keep me signed in", lockout and rate limiting; sign-out
- Rotating refresh tokens with reuse detection; HttpOnly cookies (web), keychain (desktop)
- Forgot/reset password (single-use, 1-hour links, no account enumeration)
- Profile, preferences (theme, date and number formats), email change with confirmation, password
  change, active sessions and sign-out of other devices, account activity
- Data export (JSON) and account deletion
- Businesses: guided onboarding (identity, contact, GST/NZBN with check-digit validation, financial
  year, logo), several businesses per user with a switcher, business and invoice settings, audit log
- Dashboard: set-up checklist, business summary, recent activity

## Phase 2 — Core bookkeeping ✅

- Chart of accounts and journal (dual-basis ledger, see [financial model](../architecture/financial-model.md))
- Customers with search, filters and history
- Invoices: line items, GST per line, numbering, drafts, issue, statuses, overdue, cancellation by reversal
- Payments: full/partial, multiple, edits with reversal and audit
- Quotes: create, send, accept/reject/expire, convert to invoice
- PDF invoices and quotes; email delivery with history (provider abstraction already in place)
- Other income; expenses with categories, GST treatment, business/private and deductible portions
- Receipt attachments (PDF/JPEG/PNG/WebP) via the storage abstraction
- Dashboard financial cards and charts

Known limitations: documents use the business's currency (foreign-currency invoicing needs exchange
rates in the ledger and is planned); overpayments and credit notes are not supported yet (payments
can't exceed the balance; corrections are edits or cancellations with reversal).

## Phase 3 — Mileage ✅

- Vehicles (registration, make, model, year, type, fuel, default rate, default vehicle, archive)
- Mileage rates as the user's own data, with dated versions; a trip uses the version in force on its
  date and keeps it, or a rate entered for that trip
- Trips by distance or odometer readings, with start, destination, purpose and business share
- Claims calculated by the API (business km × rate) and posted to the ledger as an expense
  (Dr Mileage claims · Cr Owner funds), reversed on edit or deletion
- Mileage report: totals, business and personal kilometres, claims, by month and by vehicle, for any
  period (annual = a financial year); dashboard card and quick action
- CSV export (UTF-8 with BOM, opens in Excel)

Known limitations: one flat rate per version (IRD's two-tier kilometre rates aren't applied
automatically: add a version or enter the rate when the tier changes); no GST is claimed on mileage;
no GPS tracking or automatic trip detection.

## Phase 4 — Allocations ✅

Planning only: Solo records where the user intends money to go and never moves money.

- Destinations: nine defaults per business (income tax, GST, ACC levies, savings, emergency fund,
  KiwiSaver, investments, mortgage, personal spending) and custom ones; rename, reorder, archive;
  targets and minimum balances
- Rules in priority order, each applied to what is left: a percentage (of the amount received or of
  what remains) or a fixed amount; conditions on source (invoice payments, income, sales or other
  income), customer and an active date range; a maximum per receipt, caps per week, fortnight, month
  or custom period, and stopping at the destination's target
- Recording an invoice payment or income proposes a split (calculated by the API); the user reviews,
  adjusts and confirms it, or skips it
- Balances per destination with target progress; a history of every allocation; edits and deletions
  of receipts reverse confirmed allocations with linked reversing entries and re-plan; the user can
  reverse one to correct it
- Allocations page (overview, history, rules with a preview, destinations) and a dashboard card with
  the distribution as a chart and a table

Known limitations: money received while there are no active rules isn't allocated later (SL-409);
spending from an allocation isn't recorded yet (SL-408); a minimum balance is a flag, not a rule;
no allocation report or export yet (Phase 5).

## Phase 5 — Reports ✅

- A Reports area with shared filters (this or last month and financial year, a custom range,
  customer, category, vehicle, status) and the cash or accrual basis
- Profit and loss with the previous period; income summary (month, customer, source, financial year,
  paid versus outstanding invoices); expense summary (category, month, supplier, GST claimed, business
  versus private)
- Statement of account per customer (from the customer's page too), accounts receivable with ageing
  buckets, financial statement by month with a summary, GST summary with every record behind it
  (informational, not a return), mileage and allocations reports
- Every report as PDF (branded), CSV and Excel from one export layer; figures from the journal, shared
  with the dashboard ([ADR 0011](../decisions/0011-reports.md))

Known limitations: gross profit equals sales revenue (no cost-of-sales accounts yet, SL-512);
statements aren't emailed from the app yet (SL-513); receivables are as at today, not a past date; no
accountant export pack yet (SL-511).

## Phase 6 — Insights ✅

Deterministic, explainable rules on the report figures, calculated on request; nothing is shown
without enough data behind it ([ADR 0012](../decisions/0012-insights.md)).

- Financial performance: revenue and expenses month on month, revenue this financial year against
  last, three-month growth, the profit trend and the average month
- Business health: revenue concentration and top customers, customers with overdue balances, average
  invoice value and time to be paid, unpaid invoices over six months, expenses against revenue
- Spending: biggest categories, growing categories, unusual spending last month, the monthly pattern
  and regular commitments
- Cash flow: what's owed and received (recorded), what's set aside (planned), expected payments and
  estimated available funds (estimates, marked as such)
- Notices in plain language on the dashboard and the Insights page, dismissible per person and shown
  again when their facts change

Known limitations: notices aren't emailed yet (SL-606); insights use the business's reporting basis
only; history-based insights need a few months of records.

## Annual accounts ✅

Built against a real sole trader's accountant-prepared annual reports for the years to 31 March 2025 and 2026;
the tests check the same reports, figure by figure, for a made-up business
([ADR 0013](../decisions/0013-annual-accounts.md)).

- Tax withheld from payments, drawings, funds introduced, and GST and income tax paid or refunded
- Fixed assets with yearly depreciation (diminishing value or straight line, at the user's rates)
- Opening balances from the accountant's last balance sheet
- Balance sheet, statement of changes in equity and depreciation schedule; non-deductible expenses as
  adjustments in profit and loss

Known limitations: no asset disposals yet (SL-520); the first year's comparative puts earlier profit in
retained earnings.

## Local-first desktop app ✅

Install and go: the desktop app keeps the books on the user's computer, with its own PostgreSQL and
API ([ADR 0014](../decisions/0014-local-first-desktop.md)). It is for one person, with no account or
sign-in, and the cloud-server option and server deployment were removed
([ADR 0016](../decisions/0016-one-person-no-sign-in.md), SL-018). Signed in-app updates (SL-017).

Backup and restore of the books, with a reminder when the last backup is old (SL-013). Known
limitation: no email from the desktop app (SL-015).

## GST returns ✅

Each GST period with IRD's due date, what to pay and what was paid, on the business's GST basis, and what
is owed now (SL-019). Known limitations: public holidays aren't in the due dates, and returns are filed
with IRD outside the app.

## Phase 7 — Improvements and integrations (in progress)

Done: recurring invoices (SL-701), invoice reminders (SL-702), advanced exports (SL-704), Business
health (SL-709) and a demo business to explore on first run (SL-710). Parked for later: imports (SL-703), receipt OCR (SL-705), bank feeds (SL-706), online
invoice payments (SL-707) and a mobile app (SL-708).

## Phase 8 — Tax (in progress)

Done: GST return preparation (SL-802), income tax and ACC estimates (SL-801), provisional tax
(SL-805), entertainment at 50% (SL-806), bad debts (SL-807), the GST registration reminder (SL-808),
second-hand goods and change of use (SL-809), home office (SL-810) and low-value assets (SL-811).
Parked for later: income tax return preparation (SL-803) and filing integrations (SL-804).
