-- Solo — invoices, payments, quotes and the email history of documents.
--
-- Totals are calculated by the API (services/api/src/domain/documents.rs) and stored with each line
-- and document, so issued documents never change when rates or rules change later.

-- ─── Invoices ───────────────────────────────────────────────────────────────
CREATE TABLE invoices (
  id                  uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id         uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  customer_id         uuid NOT NULL,
  -- Assigned when the invoice is issued, from business_settings (drafts have none).
  number              text CHECK (number IS NULL OR length(number) BETWEEN 1 AND 40),
  status              text NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'issued', 'cancelled')),
  issue_date          date NOT NULL,
  due_date            date NOT NULL,
  reference           text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  currency            char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  prices_include_gst  boolean NOT NULL DEFAULT false,
  notes               text NOT NULL DEFAULT '' CHECK (length(notes) <= 4000),
  footer              text NOT NULL DEFAULT '' CHECK (length(footer) <= 2000),
  subtotal            bigint NOT NULL DEFAULT 0 CHECK (subtotal >= 0),
  discount_total      bigint NOT NULL DEFAULT 0 CHECK (discount_total >= 0),
  gst_total           bigint NOT NULL DEFAULT 0 CHECK (gst_total >= 0),
  total               bigint NOT NULL DEFAULT 0 CHECK (total >= 0),
  quote_id            uuid,
  issued_at           timestamptz,
  sent_at             timestamptz,
  cancelled_at        timestamptz,
  cancellation_reason text CHECK (cancellation_reason IS NULL OR length(cancellation_reason) <= 500),
  created_by          uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id),
  CHECK (due_date >= issue_date),
  CHECK ((status = 'draft') = (number IS NULL)),
  CHECK (total = subtotal + gst_total)
);
SELECT enable_tenant_isolation('invoices');
CREATE UNIQUE INDEX invoices_number_uq ON invoices (business_id, number) WHERE number IS NOT NULL;
CREATE INDEX invoices_status_idx ON invoices (business_id, status, due_date);
CREATE INDEX invoices_customer_idx ON invoices (business_id, customer_id);
CREATE INDEX invoices_issue_date_idx ON invoices (business_id, issue_date);
GRANT SELECT, INSERT, UPDATE, DELETE ON invoices TO solo_app;

CREATE TABLE invoice_items (
  id               uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id      uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  invoice_id       uuid NOT NULL,
  position         smallint NOT NULL CHECK (position >= 0),
  description      text NOT NULL CHECK (length(btrim(description)) BETWEEN 1 AND 1000),
  quantity         numeric(12, 4) NOT NULL CHECK (quantity > 0),
  unit_price       bigint NOT NULL CHECK (unit_price >= 0),
  discount_percent numeric(5, 2) NOT NULL DEFAULT 0 CHECK (discount_percent BETWEEN 0 AND 100),
  tax_treatment    text NOT NULL CHECK (tax_treatment IN ('standard', 'zero_rated', 'exempt', 'no_gst')),
  tax_rate         numeric(7, 6) NOT NULL DEFAULT 0 CHECK (tax_rate >= 0 AND tax_rate < 1),
  discount_amount  bigint NOT NULL DEFAULT 0 CHECK (discount_amount >= 0),
  net_amount       bigint NOT NULL CHECK (net_amount >= 0),
  tax_amount       bigint NOT NULL CHECK (tax_amount >= 0),
  total_amount     bigint NOT NULL CHECK (total_amount >= 0),
  created_at       timestamptz NOT NULL DEFAULT now(),
  updated_at       timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, invoice_id) REFERENCES invoices (business_id, id) ON DELETE CASCADE,
  UNIQUE (invoice_id, position),
  CHECK (total_amount = net_amount + tax_amount)
);
SELECT enable_tenant_isolation('invoice_items');
GRANT SELECT, INSERT, UPDATE, DELETE ON invoice_items TO solo_app;

-- Payments the user records against an invoice. No money moves through the app.
CREATE TABLE invoice_payments (
  id           uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id  uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  invoice_id   uuid NOT NULL,
  payment_date date NOT NULL,
  amount       bigint NOT NULL CHECK (amount > 0),
  method       text NOT NULL CHECK (method IN ('bank_transfer', 'cash', 'card', 'cheque', 'online', 'other')),
  reference    text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  notes        text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  -- Deleting a payment voids it (its ledger entries are reversed); the row stays for the audit trail.
  voided_at    timestamptz,
  void_reason  text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by   uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at   timestamptz NOT NULL DEFAULT now(),
  updated_at   timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, invoice_id) REFERENCES invoices (business_id, id)
);
SELECT enable_tenant_isolation('invoice_payments');
CREATE INDEX invoice_payments_invoice_idx ON invoice_payments (invoice_id) WHERE voided_at IS NULL;
CREATE INDEX invoice_payments_date_idx ON invoice_payments (business_id, payment_date);
GRANT SELECT, INSERT, UPDATE ON invoice_payments TO solo_app;

-- ─── Quotes ─────────────────────────────────────────────────────────────────
CREATE TABLE quotes (
  id                   uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id          uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  customer_id          uuid NOT NULL,
  number               text NOT NULL CHECK (length(number) BETWEEN 1 AND 40),
  status               text NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'sent', 'accepted', 'rejected')),
  issue_date           date NOT NULL,
  expiry_date          date NOT NULL,
  reference            text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  currency             char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  prices_include_gst   boolean NOT NULL DEFAULT false,
  notes                text NOT NULL DEFAULT '' CHECK (length(notes) <= 4000),
  footer               text NOT NULL DEFAULT '' CHECK (length(footer) <= 2000),
  subtotal             bigint NOT NULL DEFAULT 0 CHECK (subtotal >= 0),
  discount_total       bigint NOT NULL DEFAULT 0 CHECK (discount_total >= 0),
  gst_total            bigint NOT NULL DEFAULT 0 CHECK (gst_total >= 0),
  total                bigint NOT NULL DEFAULT 0 CHECK (total >= 0),
  sent_at              timestamptz,
  decided_at           timestamptz,
  converted_invoice_id uuid,
  created_by           uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at           timestamptz NOT NULL DEFAULT now(),
  updated_at           timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  UNIQUE (business_id, number),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id),
  FOREIGN KEY (business_id, converted_invoice_id) REFERENCES invoices (business_id, id),
  CHECK (expiry_date >= issue_date),
  CHECK (total = subtotal + gst_total)
);
SELECT enable_tenant_isolation('quotes');
CREATE INDEX quotes_status_idx ON quotes (business_id, status, expiry_date);
CREATE INDEX quotes_customer_idx ON quotes (business_id, customer_id);
GRANT SELECT, INSERT, UPDATE, DELETE ON quotes TO solo_app;

ALTER TABLE invoices ADD CONSTRAINT invoices_quote_fk FOREIGN KEY (business_id, quote_id) REFERENCES quotes (business_id, id);

CREATE TABLE quote_items (
  id               uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id      uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  quote_id         uuid NOT NULL,
  position         smallint NOT NULL CHECK (position >= 0),
  description      text NOT NULL CHECK (length(btrim(description)) BETWEEN 1 AND 1000),
  quantity         numeric(12, 4) NOT NULL CHECK (quantity > 0),
  unit_price       bigint NOT NULL CHECK (unit_price >= 0),
  discount_percent numeric(5, 2) NOT NULL DEFAULT 0 CHECK (discount_percent BETWEEN 0 AND 100),
  tax_treatment    text NOT NULL CHECK (tax_treatment IN ('standard', 'zero_rated', 'exempt', 'no_gst')),
  tax_rate         numeric(7, 6) NOT NULL DEFAULT 0 CHECK (tax_rate >= 0 AND tax_rate < 1),
  discount_amount  bigint NOT NULL DEFAULT 0 CHECK (discount_amount >= 0),
  net_amount       bigint NOT NULL CHECK (net_amount >= 0),
  tax_amount       bigint NOT NULL CHECK (tax_amount >= 0),
  total_amount     bigint NOT NULL CHECK (total_amount >= 0),
  created_at       timestamptz NOT NULL DEFAULT now(),
  updated_at       timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, quote_id) REFERENCES quotes (business_id, id) ON DELETE CASCADE,
  UNIQUE (quote_id, position),
  CHECK (total_amount = net_amount + tax_amount)
);
SELECT enable_tenant_isolation('quote_items');
GRANT SELECT, INSERT, UPDATE, DELETE ON quote_items TO solo_app;

-- ─── Email history ──────────────────────────────────────────────────────────
CREATE TABLE email_logs (
  id                 uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id        uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  document_type      text NOT NULL CHECK (document_type IN ('invoice', 'quote')),
  document_id        uuid NOT NULL,
  to_email           citext NOT NULL,
  subject            text NOT NULL CHECK (length(subject) <= 300),
  status             text NOT NULL CHECK (status IN ('sent', 'failed')),
  provider_reference text CHECK (provider_reference IS NULL OR length(provider_reference) <= 500),
  error              text CHECK (error IS NULL OR length(error) <= 1000),
  sent_by            uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at         timestamptz NOT NULL DEFAULT now()
);
SELECT enable_tenant_rls('email_logs');
CREATE INDEX email_logs_document_idx ON email_logs (business_id, document_type, document_id, created_at DESC);
GRANT SELECT, INSERT ON email_logs TO solo_app;
