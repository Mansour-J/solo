# 0002 — Money as integer minor units

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

Financial figures must be exact, reproducible and add up on every document and report. JavaScript
numbers are IEEE floats; PostgreSQL `numeric` and Rust `Decimal` are exact but serialise as strings.

## Decision

- Store and transmit money as **integers in the currency's minor unit** (`bigint`, `i64`, JSON
  number) next to an explicit ISO 4217 currency code. Precision comes from a currency table.
- Store rates, quantities and percentages as `numeric` (`rust_decimal::Decimal`, JSON strings).
- Convert decimals to money only through `round_to_minor` (half away from zero) at defined points
  (per invoice line, per portion); split amounts with the largest-remainder method.
- All authoritative calculations happen in the API's `domain` module. Clients format, never compute.
- CI rejects any `real`/`double precision` column.

## Consequences

- Clients parse user input with a string-based parser (`parseMoney`) and display with `Intl`.
- Amounts are safe up to about ±90 trillion dollars (2^53 cents), far beyond need.
- Currencies with three decimals would need a larger exponent but no schema change.

## Alternatives considered

- **`numeric` everywhere, strings in JSON:** exact, but every client needs a decimal library and
  sums become string arithmetic.
- **Floats:** rounding errors in totals and GST. Rejected outright.
