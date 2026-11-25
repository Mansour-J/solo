-- Solo — recurring invoices: a schedule made from an invoice, repeating its lines. When the app opens,
-- each schedule creates the invoices that have come due (as drafts, or issued if it says so) and moves
-- on to its next date.

CREATE TABLE recurring_invoices (
  id                 uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id        uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  customer_id        uuid NOT NULL,
  name               text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 120),
  frequency          text NOT NULL CHECK (frequency IN ('weekly', 'fortnightly', 'monthly', 'quarterly', 'yearly')),
  -- The day of the month monthly, quarterly and yearly schedules keep to (the 31st becomes each
  -- month's last day).
  anchor_day         smallint NOT NULL CHECK (anchor_day BETWEEN 1 AND 31),
  next_date          date NOT NULL,
  end_date           date,
  -- Days from issue to due; empty uses the business's payment terms.
  payment_terms_days integer CHECK (payment_terms_days IS NULL OR payment_terms_days BETWEEN 0 AND 365),
  prices_include_gst boolean NOT NULL DEFAULT false,
  reference          text NOT NULL DEFAULT '' CHECK (length(reference) <= 100),
  notes              text NOT NULL DEFAULT '' CHECK (length(notes) <= 5000),
  footer             text NOT NULL DEFAULT '' CHECK (length(footer) <= 5000),
  -- The invoice lines to repeat (description, quantity, unit_price, discount_percent, tax_treatment).
  lines              jsonb NOT NULL CHECK (jsonb_typeof(lines) = 'array' AND jsonb_array_length(lines) BETWEEN 1 AND 200),
  auto_issue         boolean NOT NULL DEFAULT false,
  invoices_created   integer NOT NULL DEFAULT 0 CHECK (invoices_created >= 0),
  last_invoice_id    uuid,
  archived_at        timestamptz,
  created_at         timestamptz NOT NULL DEFAULT now(),
  updated_at         timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id),
  CHECK (end_date IS NULL OR end_date >= next_date OR archived_at IS NOT NULL),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('recurring_invoices');
GRANT SELECT, INSERT, UPDATE ON recurring_invoices TO solo_app;
