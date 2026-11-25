# Database (PostgreSQL 18 + Flyway)

- `migrations/V<n>__<what_changed>.sql`: the only way the schema changes. Never edit an applied migration; add one. Use the `add-migration` skill. Flyway applies them to the development database; the desktop app's local server uses the API's embedded runner (`solo_api::migrations`, same history and checksums), so keep them plain SQL without Flyway placeholders.
- `tests/schema_checks.sql`: run as the owner after migrating. It fails if a table with `business_id` lacks row-level security, if `solo_app` owns a table, if `audit_logs` is writable, if a column is floating point, or if a foreign key to `businesses` doesn't cascade.
- Schema `solo`. The owner (`solo_owner` locally) runs Flyway and backups. The API connects as `solo_app`, which owns nothing, so row-level security always applies to it.
- **Helpers defined by the migrations:**
  - `current_app_user_id()`, `current_business_id()` (from `solo.user_id` / `solo.business_id`, set per transaction by `Db::begin_business`)
  - `is_business_member()`
  - `enable_tenant_isolation(t)`: row-level security, the tenant policy and the `updated_at` trigger
  - `enable_tenant_rls(t)`: the same without the trigger, for append-only tables
  - `seed_chart_of_accounts(b)`, `seed_expense_categories(b)`.
- **Two databases:** `solo` (development) and `solo_test` (integration tests). Both are created by `infra/docker/postgres/init` when the volume is first initialised.

## Commands on this machine

There is no `.env`, and Solo's container publishes port **55432** (5432 belongs to Hermes Hub's database). Every compose command needs both `POSTGRES_PORT=55432` and `--env-file environment.example`. Without the port, compose recreates the database container on 5432 and fails.

```bash
POSTGRES_PORT=55432 docker compose --env-file environment.example up -d --wait db            # npm run db:up
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm migrate           # npm run db:migrate
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm \
  -e FLYWAY_URL=jdbc:postgresql://db:5432/solo_test migrate                                    # npm run db:test:prepare
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm migrate info      # npm run db:info
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm migrate validate  # npm run db:validate
POSTGRES_PORT=55432 docker compose --env-file environment.example exec -T db \
  psql -U solo_owner -d solo -v ON_ERROR_STOP=1 -f - < database/tests/schema_checks.sql
POSTGRES_PORT=55432 docker compose --env-file environment.example exec db psql -U solo_owner -d solo   # npm run db:psql
```

- Connection strings: the API uses `postgres://solo_app:change-me-app-password@127.0.0.1:55432/solo`, and the tests use `…/solo_test` (`TEST_DATABASE_URL`).
- `npm run db:reset` wipes every row in the local database: run it only when the user asks.
- In `psql` as the owner, row-level security doesn't apply. To see what the API sees: `SET search_path = solo; SET ROLE solo_app; SELECT set_config('solo.business_id', '<id>', false);`.
