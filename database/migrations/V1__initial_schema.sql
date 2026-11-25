-- Solo — database foundation (ADR-0003).
--
-- Flyway runs every migration with search_path = solo (FLYWAY_DEFAULT_SCHEMA), so objects are created
-- in the `solo` schema without qualification.
--
-- Conventions used by every later migration:
--   * Primary keys are UUIDv7 (time-ordered). The API usually generates them; uuidv7() is the fallback.
--   * Money is stored as bigint minor units (cents) next to an explicit currency code. Never float.
--     Rates, percentages and quantities are numeric.
--   * Every business-scoped table has business_id, created_at, updated_at and is registered with
--     enable_tenant_isolation(), which turns on row-level security and the updated_at trigger.
--   * The API connects as solo_app (never as the owner) and sets, per transaction:
--       SELECT set_config('solo.user_id', $1, true), set_config('solo.business_id', $2, true);
--     Row-level security is defence in depth: the API also checks membership on every request.
--   * Enumerations are text + CHECK constraints (easier to evolve than Postgres enum types).

CREATE EXTENSION IF NOT EXISTS citext;

-- ─── Roles ──────────────────────────────────────────────────────────────────
-- solo_app: the API. Subject to row-level security on every business-scoped table. Deployments make
-- it a LOGIN role (infra/docker/postgres/init creates it locally); it is created NOLOGIN here so the
-- migrations also run on a bare database (CI, fresh environments).
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'solo_app') THEN
    CREATE ROLE solo_app NOLOGIN;
  END IF;
END
$$;

GRANT USAGE ON SCHEMA solo TO solo_app;
-- Tables are granted explicitly in each migration (no blanket default privileges), so a table is never
-- exposed to the API by accident.
ALTER DEFAULT PRIVILEGES IN SCHEMA solo GRANT USAGE, SELECT ON SEQUENCES TO solo_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA solo GRANT EXECUTE ON FUNCTIONS TO solo_app;

-- ─── Request context ────────────────────────────────────────────────────────
-- The signed-in user and the business for this transaction, or NULL when unset (NULL matches no rows).
CREATE FUNCTION current_app_user_id() RETURNS uuid
  LANGUAGE sql STABLE
AS $$
  SELECT nullif(current_setting('solo.user_id', true), '')::uuid
$$;

CREATE FUNCTION current_business_id() RETURNS uuid
  LANGUAGE sql STABLE
AS $$
  SELECT nullif(current_setting('solo.business_id', true), '')::uuid
$$;

CREATE FUNCTION touch_updated_at() RETURNS trigger
  LANGUAGE plpgsql
AS $$
BEGIN
  NEW.updated_at := now();
  RETURN NEW;
END
$$;

-- Turns on row-level security for a business-scoped table, adds the tenant policy and the updated_at
-- trigger. Every table with a business_id column must be registered with this function.
CREATE FUNCTION enable_tenant_isolation(tbl regclass) RETURNS void
  LANGUAGE plpgsql
AS $$
BEGIN
  EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tbl);
  EXECUTE format(
    'CREATE POLICY tenant_isolation ON %s USING (business_id = current_business_id()) WITH CHECK (business_id = current_business_id())',
    tbl
  );
  EXECUTE format('CREATE TRIGGER touch_updated_at BEFORE UPDATE ON %s FOR EACH ROW EXECUTE FUNCTION touch_updated_at()', tbl);
END
$$;
