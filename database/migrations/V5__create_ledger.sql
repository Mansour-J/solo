-- Solo — double-entry ledger (docs/architecture/financial-model.md).
--
-- Every financial event posts a balanced journal entry. Entries are immutable: the API may only
-- insert them; corrections are reversing entries. Reports read only from here.

-- Row-level security for immutable business tables (no updated_at, so no touch trigger).
CREATE FUNCTION enable_tenant_rls(tbl regclass) RETURNS void
  LANGUAGE plpgsql
AS $$
BEGIN
  EXECUTE format('ALTER TABLE %s ENABLE ROW LEVEL SECURITY', tbl);
  EXECUTE format(
    'CREATE POLICY tenant_isolation ON %s USING (business_id = current_business_id()) WITH CHECK (business_id = current_business_id())',
    tbl
  );
END
$$;

-- ─── Chart of accounts ──────────────────────────────────────────────────────
CREATE TABLE financial_accounts (
  id           uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id  uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  code         text NOT NULL CHECK (code ~ '^[0-9]{3,6}$'),
  name         text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
  account_type text NOT NULL CHECK (account_type IN ('asset', 'liability', 'equity', 'income', 'expense')),
  -- Stable key for accounts the application posts to directly (bank, gst, sales…).
  system_key   text CHECK (system_key ~ '^[a-z_]+$'),
  archived_at  timestamptz,
  created_at   timestamptz NOT NULL DEFAULT now(),
  updated_at   timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  UNIQUE (business_id, code),
  UNIQUE (business_id, system_key)
);
SELECT enable_tenant_isolation('financial_accounts');
GRANT SELECT, INSERT, UPDATE ON financial_accounts TO solo_app;

-- Default accounts every business starts with. Expense accounts are created with expense categories.
CREATE FUNCTION seed_chart_of_accounts(b uuid) RETURNS void
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
  ON CONFLICT (business_id, system_key) DO NOTHING
$$;

-- ─── Journal ────────────────────────────────────────────────────────────────
CREATE TABLE journal_entries (
  id                uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id       uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  entry_date        date NOT NULL,
  -- Which report basis includes the entry: both, accrual only, or cash only.
  basis             text NOT NULL CHECK (basis IN ('both', 'accrual', 'cash')),
  source_type       text NOT NULL CHECK (source_type IN ('invoice', 'invoice_payment', 'expense', 'income', 'adjustment')),
  source_id         uuid,
  description       text NOT NULL CHECK (length(description) <= 300),
  currency          char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  -- Set on a reversing entry; an entry can be reversed at most once.
  reverses_entry_id uuid UNIQUE,
  created_by        uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at        timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, reverses_entry_id) REFERENCES journal_entries (business_id, id)
);
SELECT enable_tenant_rls('journal_entries');
CREATE INDEX journal_entries_source_idx ON journal_entries (business_id, source_type, source_id);
CREATE INDEX journal_entries_date_idx ON journal_entries (business_id, entry_date);
-- Append-only: no UPDATE or DELETE for the API.
GRANT SELECT, INSERT ON journal_entries TO solo_app;

CREATE TABLE journal_entry_lines (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  entry_id    uuid NOT NULL,
  account_id  uuid NOT NULL,
  debit       bigint NOT NULL DEFAULT 0 CHECK (debit >= 0),
  credit      bigint NOT NULL DEFAULT 0 CHECK (credit >= 0),
  -- Optional dimension for receivables and income by customer (constraint added with customers).
  customer_id uuid,
  FOREIGN KEY (business_id, entry_id) REFERENCES journal_entries (business_id, id) ON DELETE CASCADE,
  FOREIGN KEY (business_id, account_id) REFERENCES financial_accounts (business_id, id),
  -- Exactly one side carries an amount.
  CHECK ((debit = 0) <> (credit = 0))
);
SELECT enable_tenant_rls('journal_entry_lines');
CREATE INDEX journal_entry_lines_entry_idx ON journal_entry_lines (entry_id);
CREATE INDEX journal_entry_lines_account_idx ON journal_entry_lines (business_id, account_id);
GRANT SELECT, INSERT ON journal_entry_lines TO solo_app;

-- Every entry must balance (debits = credits) with at least two lines, checked at commit.
CREATE FUNCTION check_journal_entry_balanced() RETURNS trigger
  LANGUAGE plpgsql
AS $$
DECLARE
  difference bigint;
  line_count integer;
BEGIN
  SELECT coalesce(sum(debit) - sum(credit), 0), count(*) INTO difference, line_count
  FROM journal_entry_lines WHERE entry_id = NEW.id;
  IF line_count < 2 OR difference <> 0 THEN
    RAISE EXCEPTION 'journal entry % is unbalanced (difference %, % lines)', NEW.id, difference, line_count;
  END IF;
  RETURN NULL;
END
$$;
CREATE CONSTRAINT TRIGGER journal_entry_balanced
  AFTER INSERT ON journal_entries
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION check_journal_entry_balanced();

-- Existing businesses get the default chart of accounts.
SELECT seed_chart_of_accounts(id) FROM businesses;
