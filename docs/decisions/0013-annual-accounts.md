# 0013 — Annual accounts: owner movements, fixed assets, opening balances and a two-year data set

- **Status:** Accepted
- **Date:** 2026-11-21

## Context

A sole trader hands their accountant a year of records, and the accountant compiles annual accounts
for tax: profit and loss with non-deductible adjustments, a balance sheet, a statement of changes in
equity and a depreciation schedule. Rebuilding a real annual report (a contractor's year to
31 March 2025) showed what Solo was missing:

- tax withheld from payments by the customer (schedular payments), which is the owner's tax, not a
  bank receipt;
- drawings, funds introduced, and GST and income tax paid to or refunded by IRD;
- fixed assets depreciated each year at the owner's rates (diminishing value or straight line);
- costs that aren't deductible (IRD penalties);
- balances brought forward from the accountant's last balance sheet.

## Decision

- **Everything posts to the journal**, through `domain::ledger` builders and `services::ledger::reconcile`,
  so reports read only journal lines as before. New system accounts (seeded and backfilled in V13):
  property, plant and equipment (1500s), withholding tax, income tax, personal accounts and retained
  earnings (equity, 3200–3900s) and depreciation (6100s). Codes are taken from free ranges so they
  can't clash with custom categories.
- **Withholding tax is part of a payment:** `invoice_payments.withholding_tax` (less than the
  amount). The payment settles the full amount: Dr bank (amount − withheld), Dr withholding tax.
- **Money in and out** (`cash_movements`): drawing, funds introduced, GST payment or refund, income tax
  payment or refund. Each posts bank against the owner or IRD account; deleting keeps the record with
  a reason and reverses the posting.
- **Fixed assets** (`fixed_assets`, `depreciation_entries`). Rates are the user's own, never constants.
  An asset bought after the opening balances posts its purchase (cost and claimable GST). One held
  before carries its book value at that date, included in the opening entry. Depreciation is posted
  per financial year (`POST /assets/depreciation`): the opening value is last year's closing value,
  the brought-forward value in the first year, or the cost for months owned (counting the month of
  purchase) in the year bought. Private use is posted to drawings. A missing earlier year is a
  conflict rather than a guess; re-posting a year replaces it and removes later years, which depend
  on it. An asset with posted depreciation can't be edited until that year is removed.
- **Opening balances**: one row per business, entered as on the balance sheet (drawings and taxes
  zero or negative). Retained earnings is the balancing figure, so the entry always balances.
- **Non-deductible categories** (`financial_accounts.non_deductible`) appear under "Adjustments"
  below "Net profit before adjustments", as an accountant presents them.
- **Reports:** balance sheet (as at the period end, beside the previous year end), statement of
  changes in equity and depreciation schedule. Current year earnings are this financial year's
  profit; earlier profit is in retained earnings.
- **Two years as the test oracle.** `services/api/tests/fixtures/fy2025.json` and `fy2026.json` hold
  two consecutive years of a sole trader's records. `fixtures/build.mjs` generates them and works out
  every figure the annual reports should print by simulating the posting rules independently of the
  API (rounding, GST fractions, partly private costs, depreciation), comparatives included.
  `tests/annual_reports.rs` enters both years into one business through the API and asserts every
  figure; `npm run seed:annual` loads them into a running app and prints the comparison.
- **Update (2026-11-26):** the reports were first proven against a real contractor's two
  accountant-prepared annual reports. Before the repository was made public, the fixtures were
  replaced with a made-up business (Fernleaf Consulting) with the same structure: invoices with tax
  withheld, partly private costs paid personally, non-deductible penalties, four depreciating assets
  and opening balances.
- **Partly private costs paid personally** (household internet used for a home office): only the
  business share and its claimable GST are funds introduced. The private share never went through
  the business, so it isn't a drawing. Paid from the business account, the private share is a drawing.
- **Changes in equity** lists increases (profit, funds introduced) and decreases (drawings, tax paid)
  as positive amounts with their own totals, as the accountant presents it.

## Consequences

- An accountant can be given Solo's reports instead of a spreadsheet for this kind of business.
- The opening balance puts all earlier profit in retained earnings, so the first year's comparative
  shows current year earnings of zero where the accountant's report splits them.
- Disposals, asset revaluations, provisional tax calculation and the income tax return itself are
  not covered; the reports stay informational.
- Coverage is enforced: `npm run api:coverage:unit` (domain, ≥ 90% of lines) and
  `npm run api:coverage:integration` (whole crate from `tests/*.rs` alone, ≥ 90% of lines).

## Alternatives considered

- **Manual journals:** flexible, but a sole trader shouldn't need double entry to record a drawing.
- **Depreciation calculated in the report only:** simpler, but then the balance sheet and profit
  wouldn't include it and wouldn't balance.
- **Storing the accountant's figures directly:** fast, but tests nothing about how Solo gets there.
