-- Schema integrity checks run in CI after migrating a fresh database (as the owner).
-- Each check raises an exception with a clear message, so a mistake in a new migration fails CI.
-- Behavioural tenant-isolation tests live in services/api/tests (they act as solo_app).
SET search_path = solo;

-- Every table with a business_id column has row-level security and a policy.
DO $$
DECLARE
  missing text;
BEGIN
  SELECT string_agg(c.relname, ', ') INTO missing
  FROM pg_class c
  JOIN pg_namespace n ON n.oid = c.relnamespace AND n.nspname = 'solo'
  WHERE c.relkind = 'r'
    AND EXISTS (SELECT 1 FROM pg_attribute a WHERE a.attrelid = c.oid AND a.attname = 'business_id' AND NOT a.attisdropped)
    AND (NOT c.relrowsecurity OR NOT EXISTS (SELECT 1 FROM pg_policy p WHERE p.polrelid = c.oid));
  IF missing IS NOT NULL THEN
    RAISE EXCEPTION 'Business-scoped tables without row-level security: %. Call enable_tenant_isolation().', missing;
  END IF;
END
$$;

-- The application role does not own any table (owners bypass row-level security).
DO $$
DECLARE
  owned text;
BEGIN
  SELECT string_agg(c.relname, ', ') INTO owned
  FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace AND n.nspname = 'solo'
  WHERE c.relowner = (SELECT oid FROM pg_roles WHERE rolname = 'solo_app');
  IF owned IS NOT NULL THEN
    RAISE EXCEPTION 'solo_app owns tables (%), which would bypass row-level security', owned;
  END IF;
END
$$;

-- The audit log is append-only for the application.
DO $$
BEGIN
  IF has_table_privilege('solo_app', 'solo.audit_logs', 'UPDATE') OR has_table_privilege('solo_app', 'solo.audit_logs', 'DELETE') THEN
    RAISE EXCEPTION 'solo_app must not be able to UPDATE or DELETE audit_logs';
  END IF;
END
$$;

-- Money columns are integers, never floating point.
DO $$
DECLARE
  floats text;
BEGIN
  SELECT string_agg(table_name || '.' || column_name, ', ') INTO floats
  FROM information_schema.columns
  WHERE table_schema = 'solo' AND data_type IN ('real', 'double precision');
  IF floats IS NOT NULL THEN
    RAISE EXCEPTION 'Floating-point columns are not allowed (use bigint minor units or numeric): %', floats;
  END IF;
END
$$;

-- Every foreign key to businesses cascades, so deleting a business removes all of its records.
DO $$
DECLARE
  wrong text;
BEGIN
  SELECT string_agg(conrelid::regclass::text, ', ') INTO wrong
  FROM pg_constraint
  WHERE contype = 'f' AND confrelid = 'solo.businesses'::regclass AND confdeltype <> 'c';
  IF wrong IS NOT NULL THEN
    RAISE EXCEPTION 'Foreign keys to businesses must be ON DELETE CASCADE: %', wrong;
  END IF;
END
$$;

-- With no request context, the application sees no business data.
SET ROLE solo_app;
DO $$
BEGIN
  IF (SELECT count(*) FROM solo.businesses) <> 0 OR (SELECT count(*) FROM solo.business_settings) <> 0 THEN
    RAISE EXCEPTION 'Business rows are visible without a request context';
  END IF;
END
$$;
RESET ROLE;

SELECT 'schema checks passed' AS result;
