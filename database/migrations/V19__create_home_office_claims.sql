-- Solo — home office by IRD's square metre rate option (ird.govt.nz/home-office-expenses): a yearly
-- claim of (a × b) + (c × d), where a is the year's mortgage interest, rates and rent, b the business
-- area's share of the floor area, c the business area in square metres and d IRD's rate for the year.
-- Using it rules out claiming other home costs.

-- IRD's square metre rates by income year (the year ending on `year_end`), in cents per m².
CREATE TABLE home_office_rates (
  jurisdiction char(2) NOT NULL CHECK (jurisdiction ~ '^[A-Z]{2}$'),
  year_end     date NOT NULL,
  rate         bigint NOT NULL CHECK (rate > 0),
  PRIMARY KEY (jurisdiction, year_end)
);
GRANT SELECT ON home_office_rates TO solo_app;
INSERT INTO home_office_rates (jurisdiction, year_end, rate) VALUES
  ('NZ', DATE '2021-03-31', 4475),
  ('NZ', DATE '2022-03-31', 4785),
  ('NZ', DATE '2023-03-31', 5105),
  ('NZ', DATE '2024-03-31', 5310),
  ('NZ', DATE '2025-03-31', 5560),
  ('NZ', DATE '2026-03-31', 5730);

-- One claim per business and financial year.
CREATE TABLE home_office_claims (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  year_end       date NOT NULL,
  business_area  numeric(8, 2) NOT NULL CHECK (business_area > 0),
  total_area     numeric(8, 2) NOT NULL CHECK (total_area > 0),
  -- Mortgage interest, rates and rent paid in the year (minor units).
  premises_costs bigint NOT NULL DEFAULT 0 CHECK (premises_costs >= 0),
  -- IRD's rate used (cents per m²) and the resulting claim (minor units).
  rate           bigint NOT NULL CHECK (rate > 0),
  claim          bigint NOT NULL CHECK (claim >= 0),
  notes          text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  CHECK (business_area <= total_area),
  UNIQUE (business_id, year_end),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('home_office_claims');
GRANT SELECT, INSERT, UPDATE, DELETE ON home_office_claims TO solo_app;

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
  SELECT b, first_free_account_code(b, a.lo, a.hi), a.name, a.account_type, a.system_key
  FROM (VALUES
    (6000, 6099, 'Mileage claims', 'expense', 'mileage'),
    (1500, 1599, 'Property, plant and equipment', 'asset', 'fixed_assets'),
    (3200, 3299, 'Withholding tax paid', 'equity', 'withholding_tax'),
    (3300, 3399, 'Income tax paid', 'equity', 'income_tax'),
    (3400, 3499, 'Personal accounts', 'equity', 'personal_accounts'),
    (3900, 3999, 'Retained earnings', 'equity', 'retained_earnings'),
    (6100, 6199, 'Depreciation', 'expense', 'depreciation'),
    (6200, 6299, 'Bad debts', 'expense', 'bad_debts'),
    (6300, 6399, 'Home office', 'expense', 'home_office')
  ) AS a (lo, hi, name, account_type, system_key)
  WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.system_key = a.system_key)
$$;

SELECT seed_chart_of_accounts(id) FROM businesses;

ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_source_type_check;
ALTER TABLE journal_entries ADD CONSTRAINT journal_entries_source_type_check
  CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'mileage_trip', 'cash_movement',
                         'fixed_asset', 'depreciation', 'opening_balance', 'home_office', 'adjustment'));
