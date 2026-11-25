# Architecture overview

Solo is a desktop app for one person ([ADR 0016](../decisions/0016-one-person-no-sign-in.md)) built
as a modular monolith: one Rust API, one PostgreSQL database and one React client, all running inside
the Tauri app on the user's computer ([ADR 0014](../decisions/0014-local-first-desktop.md)).
Everything that touches money or data lives in the API. There is no sign-in: every request acts as
the owner.

```
┌──────────────────────────── Desktop app (Tauri 2) ─────────────────────────────┐
│ React 19 + Mantine 9 in the webview                                            │
│   │ http://127.0.0.1:<random port>, with the per-start x-api-key               │
│   ▼                                                                            │
│ ┌────────────────────────── Rust API (Axum) ───────────────────────────┐       │
│ │ api/          handlers, routing, OpenAPI (utoipa) — thin              │       │
│ │ context.rs, middleware/  the owner, client header and key checks      │       │
│ │ services/     business rules, transactions, PDFs, files               │       │
│ │ repositories/ SQL (SQLx), one module per aggregate                    │       │
│ │ domain/       pure rules: money, GST, validation, financial years     │       │
│ └───────────────┬──────────────────────────────────────────────────────┘       │
│                 │ solo_app role (RLS applies)                                  │
│                 ▼                                                              │
│ PostgreSQL 18, bundled (services/local), data in the app's local folder        │
└────────────────────────────────────────────────────────────────────────────────┘
```

For development and end-to-end tests the same client also runs in a browser, with Vite proxying
`/api` to the local server (`npm run local:serve`).

## Repository layout

| Path | Contents |
|---|---|
| `apps/client/` | React + TypeScript web app (`src/`), Tauri desktop shell (`src-tauri/`) |
| `services/api/` | Rust API crate (`src/` layered as above, `tests/` integration tests); also applies the embedded migrations (`migrations.rs`) |
| `services/local/` | The desktop app's local server: bundled PostgreSQL, migrations and the API in one process ([ADR 0014](../decisions/0014-local-first-desktop.md)) |
| `packages/api-client/` | OpenAPI document and generated TypeScript types + typed fetch client |
| `database/` | Flyway migrations (`migrations/`) and schema checks (`tests/`) |
| `branding/` | `brand.json` and logo assets: the single source of the product identity |
| `infra/` | The development PostgreSQL's init script (`docker/postgres/init`) |
| `e2e/` | Playwright journeys against the local server |
| `docs/` | Architecture, decisions (ADRs), development, deployment, product |

The client follows the spec's feature layout: `src/features/<module>/` holds that module's pages,
components, form schemas and API hooks (`api.ts`); `src/components` is shared UI; `src/services`
is the API client and platform (Tauri) adapters.

## Request lifecycle

1. The client calls a typed operation from `@solo/api-client` (paths and bodies are type-checked
   against the OpenAPI document generated from the Rust handlers).
2. Middleware assigns a request ID, logs the request, enforces the timeout and body limit, and
   rejects state-changing requests without the `x-client-kind` header, and (on the desktop app's
   server) any request without its per-start `x-api-key`.
3. Extractors take the owner (`Owner`, attached to every request) and, for
   `/businesses/{business_id}/…` routes, check the business exists (`BusinessContext`), else 404.
4. The handler calls a service. The service validates input (field-level errors), opens a
   transaction with `Db::begin_business(owner, business)` and calls repositories.
5. PostgreSQL row-level security re-checks every row against the transaction's business, so a bug in
   step 3 or 4 still can't read or write another tenant's data.
6. Audit entries are written in the same transaction as the change.

## Multi-tenancy

The owner can keep several businesses; every financial record belongs to exactly one business.
Isolation is enforced twice (ADR-0003):

- **Application:** `BusinessContext` checks the business exists on every business-scoped request.
- **Database:** every table with `business_id` has a row-level security policy comparing it with
  `current_setting('solo.business_id')`, set per transaction. The API connects as `solo_app`, which
  owns nothing and so cannot bypass the policies. CI fails if a business table lacks a policy
  (`database/tests/schema_checks.sql`), and integration tests prove cross-tenant reads and writes fail.

## Money

Amounts are `bigint` minor units with an explicit currency code; rates and percentages are
`numeric` (`rust_decimal` in Rust). Floating point is banned (CI checks the schema). Rounding is
half away from zero at defined points only, and proportional splits use the largest-remainder method
so parts always sum to the total (ADR-0002, `services/api/src/domain/money.rs`). The
double-entry ledger that reports will read from is described in
[financial-model.md](financial-model.md).

## Branding

`branding/brand.json` plus optional `APP_*` environment variables define the name, colours, logos,
URLs, email sender and identifiers. The API reads them at start-up, the web build at build time
(Vite injects them and generates the Mantine palette from the colour), and `npm run brand:sync`
applies them to the desktop configuration and icons. No component hardcodes the product name
(ADR-0006, [docs/development/branding.md](../development/branding.md)).

## What was reused from Hermes Hub, and what differs

Solo follows the conventions established in Hermes Hub wherever they fit:

| Area | Reused from Hermes Hub |
|---|---|
| Client stack | React 19, TypeScript 6 (strict, `noUncheckedIndexedAccess`), Vite 8, Mantine 9, TanStack Query 5, Zustand 5, zod 4, React Router 8 hash router, lucide icons, Inter |
| UI patterns | Theme structure and tokens, `Page`, `SectionCard`, empty/error/skeleton states, promise-based `confirmAction`, light/dark tokens in `global.css` without `light-dark()` |
| Tauri shell | Tauri 2, keychain-backed `secure_*` commands with an owner-only file fallback, minimal capabilities, strict CSP, release profile (LTO, strip) |
| Database | PostgreSQL 18 + Flyway from Docker, schema-scoped migrations, `enable_tenant_isolation()`, non-owner application role, text + CHECK enums, append-only audit log, token hashes only |
| Tooling | ESLint flat config with React Compiler rules and zero warnings, Vitest + Testing Library, Playwright desktop + mobile projects |
| CI/CD | Separate quality, database (migrate, re-run, validate) and desktop jobs; tag-driven release with tauri-action; version script that keeps npm and Cargo in step |
| Docs | ADRs, folder-level conventions in `CLAUDE.md`, NZ/UK spelling, "no dead UI" and "never present a simulation as real" product rules |

Important differences:

| Hermes Hub | Solo | Why |
|---|---|---|
| In-app TypeScript demo server, hosted backend planned | Rust/Axum API is the only backend from day one | The spec requires a Rust API and real financial data; a demo backend would be fake data |
| Hand-written TS API contract | OpenAPI generated from Rust handlers (utoipa) → generated TS types | One source of truth; CI fails on drift |
| Single package at repo root | Monorepo: `apps/client`, `services/api`, `services/local`, `packages/api-client` | Separate API, local server and client builds |
| Prefixed text IDs (`usr_…`) | UUIDv7 | Native Postgres type, time-ordered, efficient keys |
| Workspace = tenant, RBAC with 23 permissions | Business = tenant; one owner, no roles or sign-in | One person's books on their own computer (ADR 0016) |
| Document store in the app data dir | PostgreSQL bundled in the desktop app | The same API and schema everywhere, offline, on the user's computer (ADR 0014) |

## Key risks and how they're handled

| Risk | Mitigation |
|---|---|
| Financial calculation errors | Pure domain functions with unit tests; integer money; reconciliation tests in the ledger phase |
| One business's records reaching another | Row-level security + non-owner database role + CI schema checks + cross-tenant integration tests |
| API/client drift | Generated client; CI `contract` job |
| Corrupting history when correcting records | Issued documents are changed by reversal and re-posting, never by silent edits; append-only audit log |
| Upgrades losing data | Additive migrations applied on start (`solo_api::migrations`, Flyway-compatible), CI tests upgrading from the previous version |
| Other programs reaching the books | The local server listens on 127.0.0.1 only and requires a key generated at each start that only the app's window holds |
| Rebranding later | Central brand configuration, CI check that the desktop config matches it |

See [roadmap](../product/roadmap.md) for delivery status per phase.
