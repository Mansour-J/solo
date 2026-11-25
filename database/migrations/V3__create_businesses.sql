-- Solo — businesses, membership, settings and the audit log.
--
-- A user can own several businesses. Each business has its own financial data and settings, isolated
-- by business_id (application checks + row-level security).

-- ─── Businesses ─────────────────────────────────────────────────────────────
CREATE TABLE businesses (
  id                         uuid PRIMARY KEY DEFAULT uuidv7(),
  -- Display name used in the app and on documents.
  name                       text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 200),
  -- The individual's name or registered trading name ("Jane Smith" / "Jane Smith Design").
  trading_name               text NOT NULL DEFAULT '' CHECK (length(trading_name) <= 200),
  business_type              text NOT NULL DEFAULT 'sole_trader'
                               CHECK (business_type IN ('sole_trader', 'contractor', 'freelancer', 'partnership', 'company', 'other')),
  email                      citext CHECK (email IS NULL OR (length(email) <= 254 AND email ~ '^[^@\s]+@[^@\s]+$')),
  phone                      text CHECK (phone IS NULL OR length(phone) <= 40),
  -- {line1, line2, suburb, city, region, postcode, country}; validated by the API.
  address                    jsonb NOT NULL DEFAULT '{}'::jsonb,
  -- New Zealand Business Number: 13 digits.
  nzbn                       text CHECK (nzbn IS NULL OR nzbn ~ '^[0-9]{13}$'),
  gst_registered             boolean NOT NULL DEFAULT false,
  -- IRD/GST number: 8 or 9 digits, stored without separators.
  gst_number                 text CHECK (gst_number IS NULL OR gst_number ~ '^[0-9]{8,9}$'),
  gst_registered_from        date,
  gst_basis                  text NOT NULL DEFAULT 'payments' CHECK (gst_basis IN ('invoice', 'payments', 'hybrid')),
  currency                   char(3) NOT NULL DEFAULT 'NZD' CHECK (currency ~ '^[A-Z]{3}$'),
  locale                     text NOT NULL DEFAULT 'en-NZ' CHECK (length(locale) <= 35),
  timezone                   text NOT NULL DEFAULT 'Pacific/Auckland' CHECK (length(timezone) <= 64),
  -- Financial year start (NZ default 1 April).
  financial_year_start_month smallint NOT NULL DEFAULT 4 CHECK (financial_year_start_month BETWEEN 1 AND 12),
  financial_year_start_day   smallint NOT NULL DEFAULT 1 CHECK (financial_year_start_day BETWEEN 1 AND 28),
  -- Default basis for reports; every report states the basis it used.
  reporting_basis            text NOT NULL DEFAULT 'accrual' CHECK (reporting_basis IN ('cash', 'accrual')),
  -- Storage key of the uploaded logo (see services/api/src/services/storage.rs).
  logo_object_key            text,
  onboarding_completed_at    timestamptz,
  created_by                 uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at                 timestamptz NOT NULL DEFAULT now(),
  updated_at                 timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER touch_updated_at BEFORE UPDATE ON businesses FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
GRANT SELECT, INSERT, UPDATE, DELETE ON businesses TO solo_app;

-- ─── Members ────────────────────────────────────────────────────────────────
CREATE TABLE business_members (
  business_id uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  -- owner: full control incl. deletion. Other roles are reserved for sharing with an accountant later.
  role        text NOT NULL DEFAULT 'owner' CHECK (role IN ('owner', 'admin', 'member', 'accountant')),
  created_at  timestamptz NOT NULL DEFAULT now(),
  updated_at  timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (business_id, user_id)
);
CREATE INDEX business_members_user_idx ON business_members (user_id);
CREATE TRIGGER touch_updated_at BEFORE UPDATE ON business_members FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
GRANT SELECT, INSERT, UPDATE, DELETE ON business_members TO solo_app;

-- Membership check used by policies. SECURITY DEFINER so the policy on businesses does not recurse
-- through the policy on business_members.
CREATE FUNCTION is_business_member(b uuid) RETURNS boolean
  LANGUAGE sql STABLE SECURITY DEFINER
  SET search_path = solo, pg_temp
AS $$
  SELECT EXISTS (SELECT 1 FROM business_members m WHERE m.business_id = b AND m.user_id = current_app_user_id())
$$;

ALTER TABLE businesses ENABLE ROW LEVEL SECURITY;
-- Members can see their businesses (to list them); writes need the business selected for the transaction.
CREATE POLICY business_access ON businesses
  USING (id = current_business_id() OR is_business_member(id))
  WITH CHECK (id = current_business_id());

ALTER TABLE business_members ENABLE ROW LEVEL SECURITY;
CREATE POLICY member_access ON business_members
  USING (user_id = current_app_user_id() OR business_id = current_business_id())
  WITH CHECK (business_id = current_business_id());

-- ─── Settings (one row per business) ────────────────────────────────────────
CREATE TABLE business_settings (
  business_id                   uuid PRIMARY KEY REFERENCES businesses (id) ON DELETE CASCADE,
  -- Invoice and quote numbering: prefix + zero-padded sequence (INV-0001).
  invoice_prefix                text NOT NULL DEFAULT 'INV-' CHECK (length(invoice_prefix) <= 20),
  next_invoice_number           integer NOT NULL DEFAULT 1 CHECK (next_invoice_number > 0),
  quote_prefix                  text NOT NULL DEFAULT 'QU-' CHECK (length(quote_prefix) <= 20),
  next_quote_number             integer NOT NULL DEFAULT 1 CHECK (next_quote_number > 0),
  number_padding                smallint NOT NULL DEFAULT 4 CHECK (number_padding BETWEEN 1 AND 10),
  default_payment_terms_days    integer NOT NULL DEFAULT 14 CHECK (default_payment_terms_days BETWEEN 0 AND 365),
  default_quote_validity_days   integer NOT NULL DEFAULT 30 CHECK (default_quote_validity_days BETWEEN 1 AND 365),
  default_invoice_notes         text NOT NULL DEFAULT '' CHECK (length(default_invoice_notes) <= 2000),
  default_invoice_footer        text NOT NULL DEFAULT '' CHECK (length(default_invoice_footer) <= 2000),
  payment_instructions          text NOT NULL DEFAULT '' CHECK (length(payment_instructions) <= 2000),
  bank_account_name             text NOT NULL DEFAULT '' CHECK (length(bank_account_name) <= 120),
  bank_account_number           text NOT NULL DEFAULT '' CHECK (length(bank_account_number) <= 40),
  -- Empty means "use the product's brand colour".
  invoice_accent_color          text NOT NULL DEFAULT '' CHECK (invoice_accent_color = '' OR invoice_accent_color ~ '^#[0-9a-fA-F]{6}$'),
  default_deductible_percentage numeric(5, 2) NOT NULL DEFAULT 100 CHECK (default_deductible_percentage BETWEEN 0 AND 100),
  created_at                    timestamptz NOT NULL DEFAULT now(),
  updated_at                    timestamptz NOT NULL DEFAULT now()
);
SELECT enable_tenant_isolation('business_settings');
GRANT SELECT, INSERT, UPDATE ON business_settings TO solo_app;

-- ─── Audit log (append-only) ────────────────────────────────────────────────
-- Business events have a business_id; account events (sign-in, password change) only a user_id.
CREATE TABLE audit_logs (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id uuid REFERENCES businesses (id) ON DELETE CASCADE,
  user_id     uuid REFERENCES users (id) ON DELETE SET NULL,
  action      text NOT NULL CHECK (action ~ '^[a-z_]+(\.[a-z_]+)+$'),
  entity_type text NOT NULL CHECK (length(entity_type) <= 60),
  entity_id   uuid,
  summary     text NOT NULL CHECK (length(summary) <= 500),
  -- Before/after values of the changed fields, for financial traceability.
  changes     jsonb NOT NULL DEFAULT '{}'::jsonb,
  ip_address  text CHECK (length(ip_address) <= 64),
  request_id  text CHECK (length(request_id) <= 64),
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX audit_logs_business_recent_idx ON audit_logs (business_id, created_at DESC) WHERE business_id IS NOT NULL;
CREATE INDEX audit_logs_user_recent_idx ON audit_logs (user_id, created_at DESC) WHERE business_id IS NULL;
CREATE INDEX audit_logs_entity_idx ON audit_logs (business_id, entity_type, entity_id);
ALTER TABLE audit_logs ENABLE ROW LEVEL SECURITY;
CREATE POLICY audit_access ON audit_logs
  USING (business_id = current_business_id() OR (business_id IS NULL AND user_id = current_app_user_id()))
  WITH CHECK (business_id = current_business_id() OR (business_id IS NULL AND user_id = current_app_user_id()));
-- Append-only for the API: no UPDATE or DELETE grants.
GRANT SELECT, INSERT ON audit_logs TO solo_app;
