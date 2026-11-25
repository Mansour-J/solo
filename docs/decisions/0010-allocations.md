# 0010 — Allocations: planning records outside the journal, confirmed by the user, corrected by reversal

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

Sole traders want to set aside part of every payment for income tax, GST, ACC, savings and so on.
The spec asks for destinations, percentage and fixed rules in priority order, conditions (income
source, customer, payment date, an applicable period), caps per week, fortnight, month or a custom
period, minimum balances and targets, a review step before anything is recorded, a traceable history,
and corrections that never duplicate or corrupt balances. Solo must never claim to move money.

## Decision

- **A separate, append-only ledger, not the journal.** Allocations don't change income, expenses,
  GST or bank balances, so posting them as journal entries (memo accounts) would put planning figures
  next to the books and into every report that reads journal lines. They live in their own tables:
  `allocation_destinations`, `allocation_rules`, `allocations` (one per receipt at a time) and
  `allocation_entries`. Entries are insert-only for the API (`SELECT, INSERT` grants); a destination's
  balance is the sum of its entries.
- **Rules apply in priority order to what is left.** Each rule is a percentage (of the amount
  received, or of what earlier rules left) or a fixed amount, then limited, in this order, by its
  maximum per receipt, its cap for the period containing the receipt date, the destination's target
  (when the rule stops there) and what remains. Amounts are whole minor units (`domain::money`), so
  the planned amounts plus the remainder always equal the amount received (`domain::allocations`).
- **Conditions:** source (any, invoice payments, income, and optionally sales or other income), a
  customer, and an active date range by receipt date. Cap periods are a Monday-to-Sunday week, a
  calendar month, a fortnight or a custom number of days counted from an anchor date.
- **Minimum balances are shown, not enforced.** A destination below its minimum is flagged; the
  minimum doesn't reorder rules, because a rule that silently jumps the queue would be surprising.
- **Review before recording.** Recording an invoice payment or income opens a *pending* allocation
  when the business has active rules. Its plan is calculated by the API whenever it's viewed (so caps
  and targets reflect what's been confirmed since). The user confirms it, possibly with adjusted or
  added amounts, which writes the entries; or skips it. The review form's totals come from
  `POST /allocations/{id}/calculate`.
- **Corrections reverse, never rewrite.** Changing a receipt's amount reverses a confirmed
  allocation's entries (negative entries linked with `reverses_id`, at most one per entry) and opens a
  new pending allocation that `replaces_id` the old one. Deleting a receipt reverses a confirmed
  allocation or cancels an unconfirmed one. A date change keeps the confirmed amounts. The user can
  also reverse a confirmed allocation to correct it. At most one open allocation exists per receipt
  (partial unique indexes).
- **Defaults without figures.** Every business starts with nine destinations (income tax, GST, ACC
  levies, savings, emergency fund, KiwiSaver, investments, mortgage, personal spending) and no rules
  or amounts: Solo suggests no percentages, because the right ones depend on the person.

## Consequences

- Reports and the dashboard's profit figures are unaffected by allocations; allocation balances and
  history come from allocation entries only.
- Receipts recorded while a business has no active rules get no allocation; nothing is back-filled
  when rules are added later (a follow-up, SL-409).
- Spending from an allocation (SL-408) can be added as negative entries of a third kind without
  changing the model.

## Alternatives considered

- **Memo accounts in the journal:** reuses `reconcile`, but every journal-based report would have
  to exclude them, and a balanced double entry for "money set aside" invites the idea that money moved.
- **Storing a running balance per destination:** simpler reads, but corrections would have to keep
  it in step; summing append-only entries can't drift.
- **Recording allocations automatically:** fewer clicks, but the spec asks for review, and a plan the
  user never saw would be easy to misread as something done on their behalf.
