# Rust API (`solo-api`)

Axum + SQLx 0.9 + utoipa, edition 2024. The binary has four commands (`src/main.rs`): `serve` (default, for development), `openapi` (prints the document `npm run api:client` uses), `healthcheck` and `migrate` (applies the embedded migrations to `MIGRATE_DATABASE_URL`).

- **Migrations** are embedded by `build.rs` from `database/migrations` and applied by `migrations.rs` exactly as Flyway does (the same history table and checksums), for the desktop app's local server.
- **No sign-in (ADR 0016):** `api::app(state, owner)` attaches the owner (`context::OwnerId`, from `services::users::ensure_owner`) to every request. With `API_KEY` set (the desktop app's local server), every `/api` request must send it as `x-api-key`. `/api/v1/meta` reports `email_enabled`: the server's own mail server (`Config::email_enabled`, development and tests) or the owner's saved account (`services/email_settings.rs`, Settings › Email). Sending documents uses `email_settings::mailer` and is refused when neither is set up. `GET /users/me/system-status` checks the database, storage, PDFs and email (`services/system_status.rs`).

## Layout

- `api/`: one handler module per area, nested under `/api/v1` in `api/mod.rs`. Income and the dashboard live in `api/expenses.rs`; quotes in `api/invoices.rs`. `api/extract.rs` holds `ApiJson`, `ApiQuery`, `PageParams`, `Page<T>`, `ItemPath` and `SubItemPath`.
- `context.rs`: the `Owner` and `BusinessContext` extractors (no roles: the owner can do everything).
- `middleware/`: request metadata, `require_client_header` (the `x-client-kind` check) and `require_api_key`.
- `services/`, `repositories/`, `domain/`: as in the root `CLAUDE.md`. `config/` reads the environment and the brand.
- `db.rs` has one transaction kind per scope. Commit explicitly; dropping a transaction rolls it back.
  - `Db::begin()`: the `users` table only (creating the owner)
  - `begin_user(user)`: the profile and the list of businesses
  - `begin_business(user, business)`: business data, so row-level security applies.

## Conventions

- **Repositories** take `&mut PgConnection` and rely on row-level security instead of `WHERE business_id = …`. Inserts pass `business_id`.
- **Shared service helpers** (`services/invoices.rs`): `clean` (trim and length-check into `FieldErrors`), `load_business`, `gst_context(conn, business, date)` (the versioned GST rate and registration on that date; `.effective(treatment)`), `ReasonInput`, `PaymentMethod`. A business's today is `services::businesses::business_today(&timezone)`.
- **Tax:** GST maths lives in `domain/tax.rs` (`gst_on_exclusive`, `gst_in_inclusive`). Rates come from the `tax_rates` table, never constants.
- **Errors:**
  - `AppError` variants map to the codes in `docs/architecture/api.md`.
  - `AppError::internal(e)` is logged with its cause and returned without it.
  - A record in another business is `NotFound`, never `Forbidden`.
- **Email** goes through `services/email` (SMTP, or memory in tests). **Files** go through `services/storage.rs` (local, S3 or memory); receipts are checked by their bytes, not their names.
- **PDFs:** `services/documents.rs` renders `assets/templates/document.typ` with the fonts in `assets/fonts/`, on `spawn_blocking`.
- **Lifecycle:** modules plug into business creation, export and deletion through `services/hooks.rs`, never by editing `services/businesses.rs`.
- Skills: `add-api-endpoint`, `add-migration`, `ledger-posting`.

## Tests

- **Unit tests** sit next to the code in `#[cfg(test)]` modules; domain rules get worked examples.
- **Integration tests** (`tests/*.rs`) run the real router against the Flyway-migrated `solo_test` database as `solo_app`.
  - `tests/common/mod.rs` provides `TestApp` (memory email and storage, `app.email(to, subject)`), `register` (makes a new owner), `create_business` and `TestResponse::field_error`.
  - The test router takes the owner from an `x-test-owner` header (`user.auth()`), so each test has its own owner and they run in parallel.
- Every business-scoped feature needs a cross-tenant test (`bookkeeping_records_are_isolated_between_businesses`).

```bash
export PATH="$HOME/.cargo/bin:$PATH"
export TEST_DATABASE_URL=postgres://solo_app:change-me-app-password@127.0.0.1:55432/solo_test
cargo test --manifest-path services/api/Cargo.toml --lib                        # unit tests only, no database
cargo test --manifest-path services/api/Cargo.toml --test bookkeeping income    # one integration test
npm run api:lint && npm run api:test && npm run api:client
```

Run the API locally with `POSTGRES_PORT=55432 node scripts/api-dev.mjs` (port 8000, docs at `/api/docs`). It runs `cargo run` once, so restart it after changing the API.
