-- Solo — what an accountant's annual report needs beyond day-to-day records (ADR 0013): withholding
-- tax deducted from payments, money moved in and out by the owner or to IRD, fixed assets with
-- depreciation, opening balances, and expense categories that aren't deductible.

-- ─── Accounts ───────────────────────────────────────────────────────────────
-- The first free code in [lo, hi] for a business (codes are unique per business, and custom expense
-- categories use 5000–5999).
CREATE FUNCTION first_free_account_code(b uuid, lo integer, hi integer) RETURNS text
  LANGUAGE sql STABLE
AS $$
  SELECT min(c)::text FROM generate_series(lo, hi) AS c
  WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.code = c::text)
$$;

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
    (6100, 6199, 'Depreciation', 'expense', 'depreciation')
  ) AS a (lo, hi, name, account_type, system_key)
  WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.system_key = a.system_key)
$$;

SELECT seed_chart_of_accounts(id) FROM businesses;

-- An account (expense category) whose costs aren't deductible for tax, e.g. penalties. Reports show
-- them as an adjustment below the profit before adjustments.
ALTER TABLE financial_accounts ADD COLUMN non_deductible boolean NOT NULL DEFAULT false;

ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_source_type_check;
ALTER TABLE journal_entries ADD CONSTRAINT journal_entries_source_type_check
  CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'mileage_trip', 'cash_movement',
                         'fixed_asset', 'depreciation', 'opening_balance', 'adjustment'));

-- ─── Withholding tax on payments ────────────────────────────────────────────
-- Tax the customer deducted and paid to IRD on the owner's behalf (schedular payments): the invoice
-- is settled by `amount`, of which `amount − withholding_tax` reached the bank.
ALTER TABLE invoice_payments ADD COLUMN withholding_tax bigint NOT NULL DEFAULT 0
  CHECK (withholding_tax >= 0 AND withholding_tax < amount);

-- ─── Money in and out ───────────────────────────────────────────────────────
CREATE TABLE cash_movements (
  id            uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id   uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  movement_date date NOT NULL,
  kind          text NOT NULL CHECK (kind IN ('drawing', 'funds_introduced', 'gst_payment', 'gst_refund',
                                              'income_tax_payment', 'income_tax_refund')),
  amount        bigint NOT NULL CHECK (amount > 0),
  currency      char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  description   text NOT NULL DEFAULT '' CHECK (length(description) <= 300),
  reference     text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  voided_at     timestamptz,
  void_reason   text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by    uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at    timestamptz NOT NULL DEFAULT now(),
  updated_at    timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('cash_movements');
CREATE INDEX cash_movements_date_idx ON cash_movements (business_id, movement_date) WHERE voided_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON cash_movements TO solo_app;

-- ─── Fixed assets and depreciation ──────────────────────────────────────────
CREATE TABLE fixed_assets (
  id                uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id       uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  name              text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
  asset_group       text NOT NULL CHECK (length(btrim(asset_group)) BETWEEN 1 AND 80),
  -- Cost excluding GST (minor units) and the date it was bought.
  cost              bigint NOT NULL CHECK (cost > 0),
  -- GST paid on the purchase (claimed when the purchase posts; none for an asset brought forward).
  purchase_gst      bigint NOT NULL DEFAULT 0 CHECK (purchase_gst >= 0),
  acquired_on       date NOT NULL,
  -- Diminishing value or straight line, at the user's own annual rate (e.g. 50.00 = 50%).
  method            text NOT NULL CHECK (method IN ('dv', 'sl')),
  rate              numeric(5, 2) NOT NULL CHECK (rate > 0 AND rate <= 100),
  private_use       numeric(5, 2) NOT NULL DEFAULT 0 CHECK (private_use >= 0 AND private_use < 100),
  -- For an asset brought forward: its book (adjusted tax) value at the opening balance date. Without it,
  -- the asset was bought after the opening balances and its purchase posts from the bank.
  opening_value     bigint CHECK (opening_value IS NULL OR opening_value >= 0),
  notes             text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  archived_at       timestamptz,
  created_at        timestamptz NOT NULL DEFAULT now(),
  updated_at        timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('fixed_assets');
GRANT SELECT, INSERT, UPDATE ON fixed_assets TO solo_app;

-- Depreciation posted for an asset for a financial year.
CREATE TABLE depreciation_entries (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  asset_id       uuid NOT NULL,
  year_end       date NOT NULL,
  opening_value  bigint NOT NULL CHECK (opening_value >= 0),
  purchases      bigint NOT NULL DEFAULT 0 CHECK (purchases >= 0),
  depreciation   bigint NOT NULL CHECK (depreciation >= 0),
  private_amount bigint NOT NULL DEFAULT 0 CHECK (private_amount >= 0 AND private_amount <= depreciation),
  closing_value  bigint NOT NULL CHECK (closing_value >= 0),
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  UNIQUE (asset_id, year_end),
  FOREIGN KEY (business_id, asset_id) REFERENCES fixed_assets (business_id, id) ON DELETE CASCADE,
  CHECK (closing_value = opening_value + purchases - depreciation)
);
SELECT enable_tenant_isolation('depreciation_entries');
GRANT SELECT, INSERT, UPDATE, DELETE ON depreciation_entries TO solo_app;

-- ─── Opening balances ───────────────────────────────────────────────────────
-- Balances brought forward when a business starts using the app part-way through its life (one set
-- per business). Equity amounts are as they appear on a balance sheet: negative for drawings and
-- taxes paid. Retained earnings is the figure that makes the balances balance.
CREATE TABLE opening_balances (
  id                uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id       uuid NOT NULL UNIQUE REFERENCES businesses (id) ON DELETE CASCADE,
  as_at             date NOT NULL,
  bank              bigint NOT NULL DEFAULT 0,
  -- GST owed to IRD (negative for a refund due).
  gst_payable       bigint NOT NULL DEFAULT 0,
  drawings          bigint NOT NULL DEFAULT 0 CHECK (drawings <= 0),
  funds_introduced  bigint NOT NULL DEFAULT 0 CHECK (funds_introduced >= 0),
  personal_accounts bigint NOT NULL DEFAULT 0,
  withholding_tax   bigint NOT NULL DEFAULT 0 CHECK (withholding_tax <= 0),
  income_tax        bigint NOT NULL DEFAULT 0 CHECK (income_tax <= 0),
  created_at        timestamptz NOT NULL DEFAULT now(),
  updated_at        timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('opening_balances');
GRANT SELECT, INSERT, UPDATE, DELETE ON opening_balances TO solo_app;
