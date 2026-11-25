-- Solo — mileage: vehicles, versioned mileage rates and trips.
-- A trip's claim is distance × business share × rate. Rates are user data with effective dates (never
-- constants in code); each trip keeps the rate it used, so changing a rate never changes past claims.

-- ─── Ledger ─────────────────────────────────────────────────────────────────
-- Claims post to their own expense account (Dr Mileage claims · Cr Owner funds introduced): the owner
-- paid for the vehicle, and the claim is the business's cost of using it.
-- Its code is 6000, or the next free 6xxx code: before this migration, custom expense categories
-- could take codes from 6000 up (repositories/expenses.rs now keeps them in 5000–5999).
CREATE OR REPLACE FUNCTION seed_chart_of_accounts(b uuid) RETURNS void
  LANGUAGE sql
AS $$
  INSERT INTO financial_accounts (business_id, code, name, account_type, system_key) VALUES
    (b, '1000', 'Business bank account', 'asset', 'bank'),
    (b, '1100', 'Accounts receivable', 'asset', 'accounts_receivable'),
    (b, '2100', 'GST', 'liability', 'gst'),
    (b, '3000', 'Owner funds introduced', 'equity', 'owner_funds'),
    (b, '3100', 'Owner drawings', 'equity', 'owner_drawings'),
    (b, '4000', 'Sales', 'income', 'sales'),
    (b, '4100', 'Other income', 'income', 'other_income')
  ON CONFLICT (business_id, system_key) DO NOTHING;

  INSERT INTO financial_accounts (business_id, code, name, account_type, system_key)
  SELECT b,
         (SELECT min(c)::text FROM generate_series(6000, 6999) AS c
           WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.code = c::text)),
         'Mileage claims', 'expense', 'mileage'
  ON CONFLICT (business_id, system_key) DO NOTHING;
$$;

SELECT seed_chart_of_accounts(id) FROM businesses;

ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_source_type_check;
ALTER TABLE journal_entries ADD CONSTRAINT journal_entries_source_type_check
  CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'mileage_trip', 'adjustment'));

-- ─── Mileage rates ──────────────────────────────────────────────────────────
-- A named rate (e.g. "IRD kilometre rate") with dated versions: a trip uses the version in force on
-- its date.
CREATE TABLE mileage_rates (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  name        text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 80),
  notes       text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  archived_at timestamptz,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('mileage_rates');
CREATE UNIQUE INDEX mileage_rates_name_uq ON mileage_rates (business_id, lower(name)) WHERE archived_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON mileage_rates TO solo_app;

CREATE TABLE mileage_rate_versions (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  rate_id        uuid NOT NULL,
  effective_from date NOT NULL,
  -- Amount per kilometre in the business's currency (major units), e.g. 1.0400 = $1.04/km.
  rate_per_km    numeric(10, 4) NOT NULL CHECK (rate_per_km >= 0 AND rate_per_km <= 100),
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (rate_id, effective_from),
  FOREIGN KEY (business_id, rate_id) REFERENCES mileage_rates (business_id, id) ON DELETE CASCADE
);
SELECT enable_tenant_isolation('mileage_rate_versions');
GRANT SELECT, INSERT, UPDATE, DELETE ON mileage_rate_versions TO solo_app;

-- ─── Vehicles ───────────────────────────────────────────────────────────────
CREATE TABLE vehicles (
  id              uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id     uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  name            text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 80),
  registration    text NOT NULL DEFAULT '' CHECK (length(registration) <= 12),
  make            text NOT NULL DEFAULT '' CHECK (length(make) <= 60),
  model           text NOT NULL DEFAULT '' CHECK (length(model) <= 60),
  year            smallint CHECK (year IS NULL OR year BETWEEN 1900 AND 2100),
  vehicle_type    text NOT NULL CHECK (vehicle_type IN ('car', 'van', 'ute', 'motorcycle', 'truck', 'other')),
  fuel_type       text NOT NULL CHECK (fuel_type IN ('petrol', 'diesel', 'hybrid', 'plug_in_hybrid', 'electric', 'other')),
  -- Rate used for new trips unless another is chosen.
  default_rate_id uuid,
  -- The vehicle new trips start with (at most one active per business).
  is_default      boolean NOT NULL DEFAULT false,
  notes           text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  archived_at     timestamptz,
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, default_rate_id) REFERENCES mileage_rates (business_id, id)
);
SELECT enable_tenant_isolation('vehicles');
CREATE UNIQUE INDEX vehicles_name_uq ON vehicles (business_id, lower(name)) WHERE archived_at IS NULL;
CREATE UNIQUE INDEX vehicles_default_uq ON vehicles (business_id) WHERE is_default AND archived_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON vehicles TO solo_app;

-- ─── Trips ──────────────────────────────────────────────────────────────────
CREATE TABLE mileage_trips (
  id                   uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id          uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  vehicle_id           uuid NOT NULL,
  trip_date            date NOT NULL,
  start_location       text NOT NULL DEFAULT '' CHECK (length(start_location) <= 200),
  destination          text NOT NULL DEFAULT '' CHECK (length(destination) <= 200),
  purpose              text NOT NULL CHECK (length(btrim(purpose)) BETWEEN 1 AND 300),
  -- Odometer readings when known; otherwise only the distance is entered.
  start_odometer       numeric(9, 1) CHECK (start_odometer >= 0),
  end_odometer         numeric(9, 1),
  distance_km          numeric(8, 1) NOT NULL CHECK (distance_km > 0),
  business_percentage  numeric(5, 2) NOT NULL CHECK (business_percentage BETWEEN 0 AND 100),
  business_distance_km numeric(8, 1) NOT NULL CHECK (business_distance_km >= 0 AND business_distance_km <= distance_km),
  -- The rate the trip used: from a mileage rate (rate_id) or entered for this trip (rate_overridden).
  rate_id              uuid,
  rate_per_km          numeric(10, 4) NOT NULL CHECK (rate_per_km >= 0),
  rate_overridden      boolean NOT NULL DEFAULT false,
  currency             char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  -- business_distance_km × rate_per_km, in minor units.
  claim_amount         bigint NOT NULL CHECK (claim_amount >= 0),
  notes                text NOT NULL DEFAULT '' CHECK (length(notes) <= 2000),
  voided_at            timestamptz,
  void_reason          text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by           uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at           timestamptz NOT NULL DEFAULT now(),
  updated_at           timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, vehicle_id) REFERENCES vehicles (business_id, id),
  FOREIGN KEY (business_id, rate_id) REFERENCES mileage_rates (business_id, id),
  CHECK ((start_odometer IS NULL) = (end_odometer IS NULL)),
  CHECK (start_odometer IS NULL OR (end_odometer > start_odometer AND distance_km = end_odometer - start_odometer)),
  CHECK (rate_overridden OR rate_id IS NOT NULL)
);
SELECT enable_tenant_isolation('mileage_trips');
CREATE INDEX mileage_trips_date_idx ON mileage_trips (business_id, trip_date) WHERE voided_at IS NULL;
CREATE INDEX mileage_trips_vehicle_idx ON mileage_trips (business_id, vehicle_id);
GRANT SELECT, INSERT, UPDATE ON mileage_trips TO solo_app;
