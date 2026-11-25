---
name: add-migration
description: Add a Flyway migration to Solo — new business tables with tenant isolation (business_id cascade, composite foreign keys, row-level security, explicit grants), CHECK constraints instead of enums, integer money, applying it locally and to the test database, schema checks, and the business lifecycle hooks for seeding, export and deletion. Use for any schema change under database/migrations.
---

# Add a migration

Read `database/CLAUDE.md` and `docs/architecture/database.md` first.

## 1. File

- `database/migrations/V<n>__<what_changed>.sql`, with `n` one higher than the latest (`ls database/migrations | sort -V | tail -1`). Flyway validates the naming.
- **Never edit an applied migration.** Flyway checksums fail (`npm run db:validate`) and CI's upgrade test breaks. Add a new one.
- Start with a header comment saying what the migration does and why, like the existing ones.
- Keep it compatible with the previous API version during a rolling deploy: add a nullable column, backfill, and constrain in a later release. CI re-runs every migration (it must be a no-op) and upgrades a database from the previous version (`migrate -target=<n-1>`, then `migrate`).

## 2. A business table

```sql
CREATE TABLE things (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  customer_id uuid,
  thing_date  date NOT NULL,
  name        text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
  kind        text NOT NULL CHECK (kind IN ('standard', 'special')),
  currency    char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  amount      bigint NOT NULL CHECK (amount > 0),
  percentage  numeric(5, 2) NOT NULL DEFAULT 100 CHECK (percentage BETWEEN 0 AND 100),
  notes       text NOT NULL DEFAULT '' CHECK (length(notes) <= 2000),
  voided_at   timestamptz,
  void_reason text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by  uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id)
);
SELECT enable_tenant_isolation('things');
CREATE INDEX things_date_idx ON things (business_id, thing_date) WHERE voided_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON things TO solo_app;
```

- **Tenancy:**
  - `business_id` references `businesses (id) ON DELETE CASCADE`.
  - `SELECT enable_tenant_isolation('t')` turns on row-level security, adds the tenant policy and the `updated_at` trigger. An append-only table without `updated_at` uses `enable_tenant_rls('t')` instead (V5).
  - `UNIQUE (business_id, id)` lets other tables point at it with a composite foreign key `(business_id, x_id) REFERENCES x (business_id, id)`. A record then can't reference another business's row, even before row-level security.
  - Child tables carry `business_id` too, with `FOREIGN KEY (business_id, parent_id) REFERENCES parent (business_id, id) ON DELETE CASCADE` (`invoice_items` in V7).
- **Grants:** explicitly, only what the API needs. Financial records are voided, not deleted, so no `DELETE` (`income_records`, `expenses`). Append-only tables get `SELECT, INSERT` (`journal_entries`, `email_logs`). There are no default privileges.
- **Types:**
  - Money is `bigint` minor units plus `char(3)` currency.
  - Rates, percentages and quantities are `numeric`. No `real` or `double precision`: the schema checks fail on them.
  - Enumerations are `text` + `CHECK (… IN (…))`, never Postgres enums, so extending one is a `DROP CONSTRAINT` / `ADD CONSTRAINT` in a new migration.
  - Stored totals are guarded by CHECKs (`CHECK (net_amount = gross_amount - gst_amount)` in `income_records`).
- **Reference data** that changes over time (rates) is versioned by `effective_from` (and an exclusive `effective_to`), never hard-coded.
- **Views** use `WITH (security_invoker = true)` so row-level security applies (V9), plus `GRANT SELECT`.
- **New journal `source_type` or system account:** see the `ledger-posting` skill.

## 3. Apply it (this machine)

There is no `.env` here and Solo's PostgreSQL runs on port 55432, so the npm `db:*` scripts can't be used as they are. Every compose command needs `POSTGRES_PORT=55432` (otherwise compose recreates the database container on 5432) and `--env-file environment.example`:

```bash
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm migrate                  # = npm run db:migrate
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm \
  -e FLYWAY_URL=jdbc:postgresql://db:5432/solo_test migrate                                           # = npm run db:test:prepare
POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm migrate info              # = npm run db:info
POSTGRES_PORT=55432 docker compose --env-file environment.example exec -T db \
  psql -U solo_owner -d solo -v ON_ERROR_STOP=1 -f - < database/tests/schema_checks.sql               # must print "schema checks passed"
```

Applying a migration makes it immutable, so finish it before you migrate. Elsewhere (a `.env` present), the npm scripts from the root `CLAUDE.md` work as written.

## 4. Lifecycle hooks (`services/api/src/services/hooks.rs`)

- **Creation defaults:** rows every business starts with come from a SQL seed function called by `ledger::seed_business` in `on_business_created` (like `seed_chart_of_accounts` and `seed_expense_categories`). The migration also backfills existing businesses: `SELECT seed_x(id) FROM businesses;`.
- **Export:** add the table to `export_records` (`("things", export_table!("things"))`, and bump the array length). The account export must contain every business record.
- **Deletion:** rows go with the business through `ON DELETE CASCADE`. Stored files (object keys) must be listed by `stored_files` so they are deleted too.
- **Currency lock:** if the table holds financial records, include it in `has_financial_records`.

## 5. Finish

- Repository, service and API: the `add-api-endpoint` skill, including a cross-tenant integration test for the new table.
- Add the migration's row to "Tables (current)" in `docs/architecture/database.md`, and remove it from "Planned tables".
