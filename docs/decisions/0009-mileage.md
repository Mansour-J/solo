# 0009 — Mileage: user-owned dated rates, trips that keep their rate, claims in the ledger

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

Sole traders claim business use of a private vehicle per kilometre. The rate changes every year (IRD
publishes kilometre rates, in tiers, by fuel type), people use other rates too, and the spec requires
that government rates are never hard-coded, that rates are versioned by effective date, and that each
trip clearly records the rate it used. Claims must count as business expenses in every report.

## Decision

- **Rates are user data.** A `mileage_rates` row (e.g. "Kilometre rate") has dated
  `mileage_rate_versions` (rate per km in major units, 4 decimal places). Solo seeds none: the user
  enters the figure they claim. A vehicle has a default rate; a trip may choose another rate or enter
  a rate for that trip only.
- **A trip keeps its rate.** The trip stores `rate_per_km` (and `rate_id`, or `rate_overridden`) at
  save time. Editing a rate version changes future calculations only; recorded claims stay as they
  were. Re-saving a trip recalculates it with the rate in force on its date.
- **Calculation on the server.** `business km = round(distance × share, 0.1 km)`,
  `claim = round(business km × rate, cent)`, half away from zero (`domain::mileage`). The trip form
  shows the API's calculation (`POST /mileage/calculate`); clients never compute claims.
- **Claims in the ledger.** Each trip with a claim posts `Dr Mileage claims · Cr Owner funds
  introduced` on both bases (the owner paid for the vehicle). `Mileage claims` is a system expense
  account (`system_key = mileage`), separate from expense categories, so reports show it as its own
  line. Its code is 6000, or the next free 6xxx code in a business whose custom categories had
  already reached 6000; category codes now stay within 5000–5999. Edits and deletions reverse
  through `reconcile`, like every other record.
- **Exports** are CSV in UTF-8 with a byte-order mark (Excel opens it correctly), with formula-like
  text neutralised (`services::export`), reusable by the reports phase.

## Consequences

- Kilometres, claims, monthly and per-vehicle totals come from trips (the mileage report); the money
  also flows through the ledger, so profit and the dashboard include it without special cases.
- IRD's two-tier rates aren't applied automatically: the user adds a version (or enters a rate)
  when they pass the tier threshold. GST on mileage isn't claimed. Both are documented limitations.

## Alternatives considered

- **Shipping IRD rates as seed data:** they change yearly and depend on tier and fuel type; stale
  figures would silently produce wrong claims, and the spec forbids hard-coded government rates.
- **Posting claims to the "Vehicle expenses" category:** categories can be renamed or archived and
  mixing claims with actual vehicle costs hides the double-claiming risk.
- **Recomputing claims from the current rate when reporting:** history would change when a rate is
  corrected, and reports would disagree with the ledger.
