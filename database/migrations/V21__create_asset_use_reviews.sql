-- Solo — GST change-of-use adjustments (IR375): an asset costing over $10,000 excluding GST is reviewed at
-- each balance date for a number of adjustment periods (2, 5 or 10 by cost). Each review records the
-- asset's actual business use in that period; the adjustment (full GST × the change in use since
-- acquisition) posts to the GST account at the balance date.

CREATE TABLE asset_use_reviews (
  id           uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id  uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  asset_id     uuid NOT NULL,
  -- The end of the adjustment period: a balance date.
  period_end   date NOT NULL,
  -- Business (taxable) use of the asset during the period, e.g. 80.00.
  business_use numeric(5, 2) NOT NULL CHECK (business_use >= 0 AND business_use <= 100),
  notes        text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  created_at   timestamptz NOT NULL DEFAULT now(),
  updated_at   timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, asset_id) REFERENCES fixed_assets (business_id, id) ON DELETE CASCADE,
  UNIQUE (asset_id, period_end),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('asset_use_reviews');
GRANT SELECT, INSERT, UPDATE, DELETE ON asset_use_reviews TO solo_app;

ALTER TABLE journal_entries DROP CONSTRAINT journal_entries_source_type_check;
ALTER TABLE journal_entries ADD CONSTRAINT journal_entries_source_type_check
  CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'mileage_trip', 'cash_movement',
                         'fixed_asset', 'depreciation', 'opening_balance', 'home_office', 'asset_use_review',
                         'adjustment'));
