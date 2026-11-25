# Architecture decision records

One file per significant decision, `NNNN-short-title.md`, numbered in order and never renumbered.
A superseded ADR stays, marked `Superseded by NNNN`. Same format as Hermes Hub.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-stack-and-monorepo.md) | Rust/Axum API, React/Mantine client, Tauri desktop, in one monorepo; Hermes Hub conventions | Accepted |
| [0002](0002-money-as-integer-minor-units.md) | Money as integer minor units; decimals for rates; defined rounding | Accepted |
| [0003](0003-tenant-isolation.md) | Business as tenant; application checks plus PostgreSQL row-level security | Accepted |
| [0004](0004-sessions-and-tokens.md) | Opaque rotating tokens: HttpOnly cookies on the web, keychain on desktop | Superseded by 0016 |
| [0005](0005-openapi-generated-client.md) | OpenAPI generated from Rust handlers; generated TypeScript client | Accepted |
| [0006](0006-central-branding.md) | Central brand configuration with environment overrides | Accepted |
| [0007](0007-runtime-checked-sql.md) | Runtime-checked SQLx queries, verified by integration tests | Accepted |
| [0008](0008-ledger-and-pdfs.md) | Dual-basis append-only ledger with reconcile-by-reversal; Typst for PDFs | Accepted |
| [0009](0009-mileage.md) | Mileage: user-owned dated rates, trips that keep their rate, claims in the ledger | Accepted |
| [0010](0010-allocations.md) | Allocations: planning records outside the journal, confirmed by the user, corrected by reversal | Accepted |
| [0011](0011-reports.md) | Reports: one report shape, one export layer (PDF, CSV, Excel), figures from the journal | Accepted |
| [0012](0012-insights.md) | Insights: deterministic rules on report figures, calculated on request; dismissible notices | Accepted |
| [0013](0013-annual-accounts.md) | Annual accounts: owner movements, withholding tax, fixed assets and depreciation, opening balances; two years of annual accounts as the test oracle | Accepted |
| [0014](0014-local-first-desktop.md) | Local-first desktop app: embedded PostgreSQL by default, a cloud server as an option | Accepted; the cloud option removed by 0016 |
| [0015](0015-desktop-updates.md) | In-app desktop updates: signed packages, a feed address set at build time, Settings › Updates | Accepted |
| [0016](0016-one-person-no-sign-in.md) | One person, no sign-in: one owner, a per-start key on the local server, desktop only | Accepted |

## Template

```markdown
# NNNN — Title

- **Status:** Proposed | Accepted | Superseded by NNNN
- **Date:** YYYY-MM-DD

## Context
## Decision
## Consequences
## Alternatives considered
```
