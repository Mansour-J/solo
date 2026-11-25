-- Solo — customers.

CREATE TABLE customers (
  id                 uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id        uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  -- How the customer appears in lists and on documents.
  name               text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 200),
  business_name      text NOT NULL DEFAULT '' CHECK (length(business_name) <= 200),
  contact_person     text NOT NULL DEFAULT '' CHECK (length(contact_person) <= 200),
  email              citext CHECK (email IS NULL OR (length(email) <= 254 AND email ~ '^[^@\s]+@[^@\s]+$')),
  phone              text CHECK (phone IS NULL OR length(phone) <= 40),
  billing_address    jsonb NOT NULL DEFAULT '{}'::jsonb,
  business_address   jsonb NOT NULL DEFAULT '{}'::jsonb,
  nzbn               text CHECK (nzbn IS NULL OR nzbn ~ '^[0-9]{13}$'),
  tax_number         text CHECK (tax_number IS NULL OR length(tax_number) <= 40),
  notes              text NOT NULL DEFAULT '' CHECK (length(notes) <= 4000),
  -- NULL means the business's default payment terms.
  payment_terms_days integer CHECK (payment_terms_days IS NULL OR payment_terms_days BETWEEN 0 AND 365),
  default_currency   char(3) CHECK (default_currency IS NULL OR default_currency ~ '^[A-Z]{3}$'),
  is_active          boolean NOT NULL DEFAULT true,
  created_at         timestamptz NOT NULL DEFAULT now(),
  updated_at         timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('customers');
CREATE INDEX customers_name_idx ON customers (business_id, lower(name));
CREATE INDEX customers_active_idx ON customers (business_id, is_active);
GRANT SELECT, INSERT, UPDATE, DELETE ON customers TO solo_app;

-- Ledger lines can carry the customer (receivables and income by customer). A customer with
-- financial records can't be deleted, only deactivated.
ALTER TABLE journal_entry_lines
  ADD CONSTRAINT journal_entry_lines_customer_fk FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id);
CREATE INDEX journal_entry_lines_customer_idx ON journal_entry_lines (business_id, customer_id) WHERE customer_id IS NOT NULL;
