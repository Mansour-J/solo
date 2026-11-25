# 0012 — Insights: deterministic rules on report figures, calculated on request

- **Status:** Accepted
- **Date:** 2026-11-21

## Context

Phase 6 adds insights: what the figures mean (performance, business health, spending, cash flow)
and short notices such as "You have three overdue invoices". The spec asks for deterministic,
explainable rules without an AI provider, nothing shown without enough data, and forecasts clearly
separated from recorded figures. It also says insights shouldn't repeat the reports.

## Decision

- **Rules in `domain::insights`, with explicit thresholds.** Percent changes need a positive base on
  both sides; "growing" needs +20% and +$50; "unusual" needs 1.5× the usual amount, $100 more and at
  least three months of history; recurring commitments are paid in each of the last three months
  within 10% of their average; averages need at least three records. A rule that can't be supported
  returns nothing, so the insight isn't shown and its section explains what's needed instead.
- **Inputs are the report queries.** Revenue, expenses and GST-exclusive figures come from the same
  journal queries as reports and the dashboard (`repositories::reports`, on the business's reporting
  basis); payment times, receivables at past month ends and money in and out come from the records
  (`repositories::insights`). Insights add no figures of their own to the books.
- **Each insight explains itself:** a headline figure, a plain-language statement, how it was worked
  out, a tone (never the only signal), the figures behind it, a link to the records, and an
  `estimate` flag for forecasts (expected payments, estimated available funds).
- **Calculated on request, not stored.** `GET /insights` builds everything from the records, so an
  insight can't go stale or disagree with a report.
- **Notices can be dismissed per person.** `insight_dismissals` keeps a user's dismissed notice keys
  per business. A key includes what the notice says (e.g. the count and total of overdue invoices, or
  the month), so a dismissed notice comes back when its facts change. Notices are in the app only.

## Consequences

- New insights are a rule in `domain::insights` with worked tests, a few lines in
  `services/insights.rs`, and an assertion in `tests/insights.rs`.
- Insights that need history (trends, unusual spending) appear only after a few months of records.
- Emailing notices is a follow-up (SL-606): the keys and thresholds already make a digest possible.

## Alternatives considered

- **Storing insights in a table by a scheduled job:** cheaper to read, but they would lag behind
  edits and corrections, and need invalidation; the calculations are small enough to run per request.
- **A language model to phrase or find insights:** the spec rules it out for now, and deterministic
  rules can be tested and explained.
- **Showing every insight with a "not enough data" label:** noisier than leaving it out, and invites
  reading meaning into thin data.
