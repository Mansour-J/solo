-- Solo — asset disposals (selling or scrapping an asset). There is no depreciation in the year of
-- disposal (IR260): the sale is compared with the asset's adjusted tax value at the start of that
-- year. Above it, the excess up to cost is depreciation recovered (taxable income); below it, a loss
-- on disposal (deductible); above cost, a capital gain (not taxable). The private-use share stays
-- with the owner. On a GST-registered business, the sale includes GST.

CREATE TABLE asset_disposals (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  asset_id       uuid NOT NULL,
  disposed_on    date NOT NULL,
  -- What was received, including any GST (0 when scrapped), and the GST in it (minor units).
  sale_price     bigint NOT NULL CHECK (sale_price >= 0),
  sale_gst       bigint NOT NULL DEFAULT 0 CHECK (sale_gst >= 0 AND sale_gst <= sale_price),
  -- The adjusted tax value at the start of the year of disposal, and the outcome (minor units).
  book_value     bigint NOT NULL CHECK (book_value >= 0),
  recovered      bigint NOT NULL DEFAULT 0 CHECK (recovered >= 0),
  loss           bigint NOT NULL DEFAULT 0 CHECK (loss >= 0),
  capital_gain   bigint NOT NULL DEFAULT 0 CHECK (capital_gain >= 0),
  notes          text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, asset_id) REFERENCES fixed_assets (business_id, id) ON DELETE CASCADE,
  UNIQUE (asset_id),
  UNIQUE (business_id, id),
  CHECK (recovered = 0 OR loss = 0)
);
SELECT enable_tenant_isolation('asset_disposals');
GRANT SELECT, INSERT, UPDATE, DELETE ON asset_disposals TO solo_app;

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
    (6300, 6399, 'Home office', 'expense', 'home_office'),
    (4200, 4299, 'Depreciation recovered', 'income', 'depreciation_recovered'),
    (6400, 6499, 'Loss on disposal of assets', 'expense', 'loss_on_disposal')
  ) AS a (lo, hi, name, account_type, system_key)
  WHERE NOT EXISTS (SELECT 1 FROM financial_accounts f WHERE f.business_id = b AND f.system_key = a.system_key)
$$;

SELECT seed_chart_of_accounts(id) FROM businesses;

ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_source_type_check;
ALTER TABLE journal_entries ADD CONSTRAINT journal_entries_source_type_check
  CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'mileage_trip', 'cash_movement',
                         'fixed_asset', 'depreciation', 'opening_balance', 'home_office', 'asset_use_review',
                         'asset_disposal', 'adjustment'));
