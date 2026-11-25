-- Solo — expense categories, expenses with receipts, and income not tied to an invoice.

-- ─── Expense categories ─────────────────────────────────────────────────────
-- Each category posts to its own expense account, so reports by category come from the ledger.
CREATE TABLE expense_categories (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  name        text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 80),
  account_id  uuid NOT NULL,
  -- One of the defaults every business starts with (can be renamed or archived, not deleted).
  is_default  boolean NOT NULL DEFAULT false,
  archived_at timestamptz,
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, account_id) REFERENCES financial_accounts (business_id, id)
);
SELECT enable_tenant_isolation('expense_categories');
CREATE UNIQUE INDEX expense_categories_name_uq ON expense_categories (business_id, lower(name)) WHERE archived_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON expense_categories TO solo_app;

CREATE FUNCTION seed_expense_categories(b uuid) RETURNS void
  LANGUAGE plpgsql
AS $$
DECLARE
  names text[] := ARRAY['Advertising', 'Bank fees', 'Insurance', 'Internet', 'Telephone', 'Software',
                        'Computer equipment', 'Office supplies', 'Professional services', 'Rent',
                        'Vehicle expenses', 'Travel', 'Meals', 'Subcontractors', 'Utilities', 'Other'];
  i integer;
  account uuid;
BEGIN
  FOR i IN 1 .. array_length(names, 1) LOOP
    INSERT INTO financial_accounts (business_id, code, name, account_type)
    VALUES (b, (5000 + (i - 1) * 10)::text, names[i], 'expense')
    ON CONFLICT (business_id, code) DO NOTHING
    RETURNING id INTO account;
    IF account IS NOT NULL THEN
      INSERT INTO expense_categories (business_id, name, account_id, is_default) VALUES (b, names[i], account, true);
    END IF;
  END LOOP;
END
$$;

-- ─── Expenses ───────────────────────────────────────────────────────────────
CREATE TABLE expenses (
  id                       uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id              uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  expense_date             date NOT NULL,
  supplier                 text NOT NULL CHECK (length(btrim(supplier)) BETWEEN 1 AND 200),
  description              text NOT NULL DEFAULT '' CHECK (length(description) <= 1000),
  category_id              uuid NOT NULL,
  currency                 char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  -- Amount paid, including GST.
  gross_amount             bigint NOT NULL CHECK (gross_amount > 0),
  gst_treatment            text NOT NULL CHECK (gst_treatment IN ('standard', 'zero_rated', 'exempt', 'no_gst')),
  gst_amount               bigint NOT NULL DEFAULT 0 CHECK (gst_amount >= 0),
  net_amount               bigint NOT NULL CHECK (net_amount >= 0),
  payment_method           text NOT NULL CHECK (payment_method IN ('business_account', 'personal_funds', 'credit_card', 'cash', 'other')),
  payment_reference        text NOT NULL DEFAULT '' CHECK (length(payment_reference) <= 100),
  classification           text NOT NULL CHECK (classification IN ('business', 'personal', 'partial', 'rechargeable')),
  -- Share used for the business (drives the ledger split), and share the user treats as deductible.
  business_percentage      numeric(5, 2) NOT NULL CHECK (business_percentage BETWEEN 0 AND 100),
  deductible_percentage    numeric(5, 2) NOT NULL CHECK (deductible_percentage BETWEEN 0 AND 100),
  rechargeable_customer_id uuid,
  notes                    text NOT NULL DEFAULT '' CHECK (length(notes) <= 2000),
  voided_at                timestamptz,
  void_reason              text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by               uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at               timestamptz NOT NULL DEFAULT now(),
  updated_at               timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, category_id) REFERENCES expense_categories (business_id, id),
  FOREIGN KEY (business_id, rechargeable_customer_id) REFERENCES customers (business_id, id),
  CHECK (net_amount = gross_amount - gst_amount)
);
SELECT enable_tenant_isolation('expenses');
CREATE INDEX expenses_date_idx ON expenses (business_id, expense_date) WHERE voided_at IS NULL;
CREATE INDEX expenses_category_idx ON expenses (business_id, category_id);
GRANT SELECT, INSERT, UPDATE ON expenses TO solo_app;

CREATE TABLE expense_attachments (
  id           uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id  uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  expense_id   uuid NOT NULL,
  object_key   text NOT NULL CHECK (length(object_key) <= 300),
  filename     text NOT NULL CHECK (length(filename) BETWEEN 1 AND 255),
  content_type text NOT NULL CHECK (content_type IN ('application/pdf', 'image/jpeg', 'image/png', 'image/webp')),
  size_bytes   integer NOT NULL CHECK (size_bytes > 0),
  uploaded_by  uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at   timestamptz NOT NULL DEFAULT now(),
  updated_at   timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, expense_id) REFERENCES expenses (business_id, id) ON DELETE CASCADE
);
SELECT enable_tenant_isolation('expense_attachments');
CREATE INDEX expense_attachments_expense_idx ON expense_attachments (expense_id);
GRANT SELECT, INSERT, DELETE ON expense_attachments TO solo_app;

-- ─── Income not tied to an invoice ──────────────────────────────────────────
CREATE TABLE income_records (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  income_date    date NOT NULL,
  payer          text NOT NULL CHECK (length(btrim(payer)) BETWEEN 1 AND 200),
  description    text NOT NULL DEFAULT '' CHECK (length(description) <= 1000),
  income_type    text NOT NULL CHECK (income_type IN ('sales', 'other')),
  customer_id    uuid,
  currency       char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  gross_amount   bigint NOT NULL CHECK (gross_amount > 0),
  gst_treatment  text NOT NULL CHECK (gst_treatment IN ('standard', 'zero_rated', 'exempt', 'no_gst')),
  gst_amount     bigint NOT NULL DEFAULT 0 CHECK (gst_amount >= 0),
  net_amount     bigint NOT NULL CHECK (net_amount >= 0),
  payment_method text NOT NULL CHECK (payment_method IN ('bank_transfer', 'cash', 'card', 'cheque', 'online', 'other')),
  reference      text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  notes          text NOT NULL DEFAULT '' CHECK (length(notes) <= 2000),
  voided_at      timestamptz,
  void_reason    text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by     uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id),
  CHECK (net_amount = gross_amount - gst_amount)
);
SELECT enable_tenant_isolation('income_records');
CREATE INDEX income_records_date_idx ON income_records (business_id, income_date) WHERE voided_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON income_records TO solo_app;

-- Existing businesses get the default categories (and their expense accounts).
SELECT seed_expense_categories(id) FROM businesses;
