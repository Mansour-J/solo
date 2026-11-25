# Solo — working notes for Claude Code

Bookkeeping app for NZ sole traders (spec originally titled "Daikoku"). Rust/Axum API, PostgreSQL +
Flyway, React/Mantine client, Tauri 2 desktop shell. It is a desktop app for one person with no
sign-in: it runs its own PostgreSQL and the API in-process (`services/local`, ADR 0014), and every
request acts as the one owner (ADR 0016). The web build is for development and end-to-end tests only.
Conventions follow `~/projects/personal/hermes-hub` where they fit; differences are listed in
`docs/architecture/overview.md`.

## Commands

```bash
npm run check                         # tsc + eslint (0 warnings) + prettier --check + vitest
npm run api:test                      # cargo test: API (unit + integration; needs the test DB, below) and the local server
npm run local:serve                   # the desktop app's local server (embedded PostgreSQL + API) on its own
npm run api:lint                      # cargo fmt --check + clippy -D warnings
npm run db:up && npm run db:migrate && npm run db:test:prepare
npm run api:client                    # after ANY API change: regenerate packages/api-client
npm run test:e2e                      # Playwright vs Vite on http://localhost:5173 proxying `npm run local:serve`
npm run brand:sync                    # after editing branding/brand.json
npm run readme:screenshots            # regenerate docs/screenshots for the README (update-readme skill)
npm run backlog -- list open          # backlog items (also show SL-123, next phase-7, check)
```

Cargo is in `~/.cargo/bin`. Integration tests read `TEST_DATABASE_URL`
(default `postgres://solo_app:change-me-app-password@127.0.0.1:5432/solo_test`). Port 5432 is often
taken by Hermes Hub's database on this machine: run Solo's with `POSTGRES_PORT=55432`.

## Architecture rules

- **API layers:** `api` (thin handlers + utoipa docs) → `services` (rules, permissions, transactions)
  → `repositories` (SQL) → `domain` (pure logic, unit-tested). No business logic in handlers.
- **No sign-in (ADR 0016):** handlers take `Owner` or `BusinessContext` (`context.rs`); there are no
  roles or permission checks. Never add accounts, sessions or passwords back without a new ADR.
- **Tenancy:** business data only under `/api/v1/businesses/{business_id}/…` with the
  `BusinessContext` extractor, and only inside `state.db.begin_business(user, business)`. New tables:
  `business_id` FK with `ON DELETE CASCADE`, `SELECT enable_tenant_isolation('t')`, explicit grants to
  `solo_app`, and a cross-tenant integration test.
- **Money:** integer minor units (`i64`/`bigint`/JSON number); `Decimal` for rates; round only via
  `domain::money`. Clients never compute authoritative totals.
- **Migrations:** new `database/migrations/V<n>__*.sql` only; never edit an applied one. They are embedded
  in the API (`build.rs`) and applied by `solo_api::migrations` in the desktop app (Flyway for the development database).
- **SQLx 0.9** rejects `format!` SQL: build column lists with `macro_rules!` + `concat!`.
- **Errors:** return `AppError`; validation errors carry field names matching request fields so the
  client can put them on inputs (`useApiMutation({ form })`).
- **Audit:** significant changes write `audit::record` in the same transaction, with before/after.
- **Lifecycle hooks:** modules plug into business creation/export/deletion via `services/hooks.rs`.
- **Ledger:** never insert journal rows directly. A record builds the entries it should have with
  `domain::ledger` builders and calls `services::ledger::reconcile` with its sources; reconcile
  reverses what changed and posts the rest. Reports read only journal lines (`repositories/reports.rs`).
- **utoipa paths:** handler args can't be tuple patterns; use `Path(p): Path<ItemPath>` / `SubItemPath`
  (`api/extract.rs`, serde aliases map `invoice_id`, `payment_id`…).
- **PDFs:** Typst template `services/api/assets/templates/document.typ` with embedded Inter fonts.

## Client rules

- Server state only via TanStack Query; feature hooks in `features/<module>/api.ts`; mutations via
  `useApiMutation` (toasts, field errors, invalidation). After a financial change invalidate
  `financialKeys(businessId)` (features/invoices/api.ts): invoices, quotes, customers, dashboard, activity.
- Money in forms: `MoneyInput` (minor units, string parsing). Never compute authoritative totals in
  the client: the invoice editor calls `/invoices/calculate` (`useCalculation`).
- Charts: `@mantine/charts` with the validated tokens `--app-income` (blue) / `--app-expense` (orange);
  run the dataviz validator before adding colours; always a legend/labels and a table view.
- Forms: Mantine `useForm` + zod via `zodValidate<Values>(schema)`; field names = API field names.
- Mantine 9 is the only UI library. React Compiler lint: no setState in effects, hooks before early
  returns; component files export only components.
- Never hardcode the product name, colours or URLs: use `BRAND` (`src/branding/brand.ts`).
- **No dead UI and no fake data.** A nav item or button exists only when it works end to end.
- **The guide is part of every user-visible change.** New, renamed or removed screens, tabs, buttons,
  fields and rules update `features/guide/content.ts` in the same commit (`update-guide` skill).
  `guide.test.tsx` fails on a screen without a step or a `**label**` the app no longer shows, and the
  guide-check hook stops a commit of UI changes without the guide (`Guide: unchanged` in the message
  when nothing users see changed).
- NZ/UK spelling; typographic apostrophes (’) in UI copy.
- Check UI changes in light, dark and phone width (Playwright screenshots).

## Repository, CI and releases (public since 2026-11-26)

`github.com/Mansour-J/solo` is **public**. The private repository it replaced, with the full history,
pull requests and v0.2.0/v0.3.0 releases, is `Mansour-J/solo-old`: it holds the owner's real figures,
so never make it public or copy from its history.

- **Never commit personal data:** no real business records, figures, IRD/GST numbers, NZBN, addresses,
  bank accounts, client or agency names, the accountant's name or the owner's email. Test and seed data
  are made up (`services/api/tests/fixtures/build.mjs` generates the annual-accounts years for the
  invented "Fernleaf Consulting"; ADR 0013). Use IRD's example number `49091850` for GST numbers.
- **CI** (`ci.yml`) runs on every pull request and push to `main`; hosted runners are free on a public
  repository. Still run `/check` before committing.
- **Releases:** `npm run release:version -- X.Y.Z`, commit, then push an annotated tag `vX.Y.Z`:
  `release.yml` builds Linux, Windows and macOS installers and publishes the release
  (`docs/deployment/release-process.md`). GitHub does all building and releasing; nothing is built
  for release on this machine.
- **Updates:** the `TAURI_SIGNING_*` secrets aren't set on the new repository (the old one's can't be
  read back, and the key isn't on this machine). Recover the key or generate a new pair before switching
  on the feed (`UPDATER_ENDPOINT`, ADR 0015).
- Pull requests from forks need approval before their workflows run (the repository default for
  first-time contributors); never run untrusted pull requests on a self-hosted runner.

## Workstation gotchas

- A security hook blocks Bash commands containing `.env` (including `import.meta.env`) and writes to
  `.env*` paths: the template is `environment.example`; edit files mentioning `import.meta.env` with
  the Edit/Write tools; use `docker compose --env-file environment.example` in commands.
- Commit only when asked.

## Claude Code

Folder notes: `services/api/CLAUDE.md`, `services/local/CLAUDE.md`, `apps/client/CLAUDE.md`, `apps/client/src-tauri/CLAUDE.md`,
`database/CLAUDE.md` (local compose commands for port 55432), `e2e/CLAUDE.md`.

Backlog: `docs/backlog/backlog.html`, an interactive page (open it in a browser) whose embedded JSON holds
every item's status, priority and criteria; status lives only there. Read and change it with
`npm run backlog -- list|show|next|check` and the `backlog` skill; never recreate a Markdown copy. A private
artifact copy is republished after pushes to `main` (`npm run backlog:publish`; a hook in `.claude/settings.json` reminds you).

Commands (`.claude/commands/`):

- `/check [full]`: quality gate (client checks, API lint and tests, optionally Playwright).
- `/e2e [spec] [desktop|mobile]`: run and triage Playwright specs against a running stack.
- `/snap <routes>`: screenshot routes in light, dark and phone layouts (`npm run snap`, `scripts/snap.mjs`).
- `/desktop [dev|release|build|smoke]`: run, build or smoke-test the Tauri app.
- `/sync-docs`: regenerate the API client, check the brand, links, backlog, roadmap, README status and migration table.
- `/release-check [X.Y.Z]`: pre-release verification (`docs/deployment/release-process.md`).
- `/release X.Y.Z`: cut a release and watch the workflow. Only when the user asks.
- `/phase [n|item]`: plan and deliver the next roadmap phase or a named item, then update the roadmap.

Skills (`.claude/skills/`):

- `add-api-endpoint`: domain → repository → service → handler, tests, cross-tenant check, client regeneration.
- `add-migration`: Flyway migration and business table with tenant isolation, grants and lifecycle hooks.
- `ledger-posting`: journal entries for a financial record via `domain::ledger` and `reconcile`.
- `add-feature-page`: client screen, route, nav, forms, states, phone layout and tests.
- `ui-review`: screenshots plus the visual checklist before calling UI work done.
- `backlog`: read, add and update backlog items in `docs/backlog/backlog.html` (statuses, criteria, next ID, validation).
- `update-guide`: the in-app guide's steps for a new or changed screen, in the app's own words (enforced by `guide.test.tsx` and the guide-check hook).
- `update-readme`: README features, screenshots (`npm run readme:screenshots`) and status after any user-visible change.
