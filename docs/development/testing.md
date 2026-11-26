# Testing

Tests exercise real behaviour; layers whose integration matters are not mocked.

| Level | Where | Runs against | Command |
|---|---|---|---|
| Domain unit tests | `services/api/src/**` (`#[cfg(test)]`) | Pure functions | `cargo test --lib` |
| API integration | `services/api/tests/*.rs` | The full router (`tower::oneshot`), a real Flyway-migrated PostgreSQL, in-memory email and storage | `npm run api:test` |
| Schema checks | `database/tests/schema_checks.sql` | A migrated database (as owner) | see [database.md](../architecture/database.md) |
| Client unit/component | `apps/client/src/**/*.test.ts(x)` | jsdom; `fetch` stubbed only to simulate server responses | `npm test` |
| End to end | `e2e/*.spec.ts` | The running stack (web, API, database, Mailpit) in Chromium, desktop and phone viewports | `npm run test:e2e` |
| Desktop shell | `apps/client/src-tauri` | Rust unit tests (key validation, connection settings) and a launch smoke test in CI | `cargo test` |
| Local server | `services/local/tests` | The bundled PostgreSQL from an empty folder: first start, restart, recovery after a killed app | `cargo test --manifest-path services/local/Cargo.toml` |

## Integration tests

- `TestApp::new()` builds the real application from configuration with `EMAIL_PROVIDER=memory` and
  `STORAGE_BACKEND=memory`; `app.email(to, subject)` waits for an email so tests can follow links.
- Tests isolate themselves with unique users and businesses, so they run in parallel on one database.
- Every business-scoped feature needs a tenant-isolation test: a second owner (`app.register`) must get 404, and a
  direct SQL attempt inside another business's context must see and change nothing.
- Prepare the database once: `npm run db:up && npm run db:test:prepare`. Override the connection with
  `TEST_DATABASE_URL`.
- `tests/migrations.rs` connects as the owner (`TEST_OWNER_DATABASE_URL`) to build scratch databases
  with the embedded migration runner and compare them with Flyway's history.
- Each test makes its own owner (`app.register`); the test router takes it from an `x-test-owner` header.
- `tests/owner.rs` covers the owner (created once, an empty profile), the local server's `x-api-key`, and
  sending a document without a mail server.

## Coverage

Each kind of test must cover at least 90% of lines on its own (cargo-llvm-cov; install with
`cargo install cargo-llvm-cov` and `rustup component add llvm-tools-preview`):

| Command | Measures | Last measured |
|---|---|---|
| `npm run api:coverage:unit` | Unit tests (`--lib`) over `domain/`, where the rules live | 99.1% of lines |
| `npm run api:coverage:integration` | Integration tests (`tests/*.rs`, no unit tests) over the whole crate, including `main.rs` and configuration | 93.1% of lines |

### Updates (99%)

Checking for and installing updates must always work: a broken updater can't deliver its own fix. Both
sides have a 99% gate, run in CI on every pull request:

| Command | Measures | Last measured |
|---|---|---|
| `npm run desktop:coverage:updates` | `apps/client/src-tauri/tests/updates.rs` over `src/updates/mod.rs` | 100% of lines |
| `npm run coverage -w @solo/client` | Settings › Updates (`features/updates/`) and the shell calls (`services/platform.ts`) | 100% of lines, branches and functions |

- The Rust tests run the real updater plugin in a mock Tauri app against a feed on 127.0.0.1, signed with
  a throwaway minisign key as releases are. They check:
  - a newer version is offered with its date and notes;
  - the latest version, a 204 and an older version offer nothing;
  - a failed or malformed check says to try again;
  - a genuine package downloads with progress, installs and restarts;
  - a changed package, another key, an old release announced as a new version, or a signature without a
    version is refused;
  - a failed download or install keeps the update to try again.
  They also call the commands through IPC with the page's argument names, and check that
  `tauri.conf.json` ships the public key with `requireSignedVersion`.
- They're an integration test (`tests/`) because on Windows only test targets can be given the manifest a
  Tauri app needs to start (`build.rs`).
- Not measured:
  - `src/updates/system.rs`: stopping the local database, running the installer and restarting, which
    would replace the test program. It's checked by updating a real installed app
    (`docs/deployment/release-process.md`).
  - `#[tauri::command]` lines: the macro leaves an uncalled closure there.

Integration tests carry most of the weight: they mock nothing below HTTP, so they catch SQL, row-level
security, posting and serialisation bugs that unit tests can't. A new endpoint gets an integration
test for its happy path, validation, conflicts and isolation; the `maintenance.rs` suite covers
looking up, changing and removing records the module suites mostly create.

## End-to-end tests

The journeys run against Vite proxying the desktop app's local server (embedded PostgreSQL), as CI
does. Run `npx playwright install chromium` once.

```bash
npm run local:serve                                   # prints the API's URL
VITE_DEV_API_PROXY=<that URL> npm run dev
npx playwright test                                   # E2E_BASE_URL defaults to http://localhost:5173
```

There is no sign-in, and every test shares the one owner, so each test works in a business of its own
(`e2e/CLAUDE.md`).

Current journeys: opening without signing in → onboarding with GST; business and invoice settings with
activity log; settings without sign-in pages; phone layout with the navigation drawer; invoice draft →
issue → payment → dashboard; quote → accepted → invoice; invoice PDF download; customer from the
dashboard; expense with a receipt; income; vehicle and rate set-up → odometer trip → claim in the
report, CSV export and dashboard; allocation rule → income → review prompt → adjusted and confirmed
split → balance and history → income deleted → allocation reversed; reports: profit and loss on both
bases → receivables → PDF export → a customer's statement from their page; year end: a payment with tax
withheld → a drawing → an asset and its depreciation → a balance sheet that balances; insights: an
overdue invoice → dashboard notice → insight → notice dismissed everywhere; the guide.

## Financial calculations

Money, GST, rounding and allocation rules are unit-tested in `domain/` with worked examples
(including half-cent rounding and proportional splits that must sum exactly). Mileage claims are
checked end to end in `services/api/tests/mileage.rs` (rate versions by date, odometer and distance
entry, partial business share, rates entered per trip, ledger reversals, report totals and CSV). Reports are tested
with a hand-checked three-month data set (`tests/reports.rs`) and two years of annual accounts
(`tests/annual_reports.rs`): a made-up sole trader's years to 31 March 2025 and 2026 from
`tests/fixtures/fy2025.json` and `fy2026.json`, entered into one business through the API and
compared, line by line and rounded to the dollar, with the profit and loss, balance sheet, statement
of changes in equity and depreciation schedule an accountant would print, comparatives included.
`tests/fixtures/build.mjs` regenerates the fixtures and works out those figures by simulating the
posting rules independently of the API; `npm run seed:annual` loads both years into a running app.
