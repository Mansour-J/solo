-- Solo — allocations: planning where money received should go (tax, GST, savings…).
-- Allocations are planning records, not ledger entries: Solo never moves money (ADR 0010).
-- Each receipt (an invoice payment or an income record) can have an allocation, calculated from the
-- business's rules and confirmed by the user. Confirmed amounts are append-only entries; corrections
-- add reversing entries, so a destination's balance is always the sum of its entries.

-- ─── Destinations ───────────────────────────────────────────────────────────
CREATE TABLE allocation_destinations (
  id              uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id     uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  name            text NOT NULL CHECK (length(btrim(name)) BETWEEN 1 AND 80),
  kind            text NOT NULL CHECK (kind IN ('income_tax', 'gst', 'acc', 'savings', 'emergency_fund',
                                                'kiwisaver', 'investment', 'mortgage', 'personal_spending', 'custom')),
  position        integer NOT NULL DEFAULT 0,
  -- Minor units in the business's currency. A target is what the user is working towards; a minimum
  -- balance is the least they want to keep (shown as a status, never enforced on money).
  target_amount   bigint CHECK (target_amount IS NULL OR target_amount > 0),
  minimum_balance bigint CHECK (minimum_balance IS NULL OR minimum_balance > 0),
  notes           text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  archived_at     timestamptz,
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('allocation_destinations');
CREATE UNIQUE INDEX allocation_destinations_name_uq ON allocation_destinations (business_id, lower(name)) WHERE archived_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON allocation_destinations TO solo_app;

-- Default destinations, without amounts or rules: the user decides what to set aside.
CREATE FUNCTION seed_allocation_destinations(b uuid) RETURNS void
  LANGUAGE sql
AS $$
  INSERT INTO allocation_destinations (business_id, name, kind, position)
  SELECT b, d.name, d.kind, d.position::integer
  FROM unnest(
         ARRAY['Income tax', 'GST', 'ACC levies', 'Savings', 'Emergency fund', 'KiwiSaver', 'Investments',
               'Mortgage', 'Personal spending'],
         ARRAY['income_tax', 'gst', 'acc', 'savings', 'emergency_fund', 'kiwisaver', 'investment',
               'mortgage', 'personal_spending']
       ) WITH ORDINALITY AS d (name, kind, position)
  WHERE NOT EXISTS (SELECT 1 FROM allocation_destinations x WHERE x.business_id = b)
$$;

SELECT seed_allocation_destinations(id) FROM businesses;

-- ─── Rules ──────────────────────────────────────────────────────────────────
-- Applied in priority order (lowest first) to what is left of a receipt.
CREATE TABLE allocation_rules (
  id              uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id     uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  destination_id  uuid NOT NULL,
  priority        integer NOT NULL DEFAULT 0,
  -- A percentage of the amount received or of what remains, or a fixed amount.
  method          text NOT NULL CHECK (method IN ('percentage', 'fixed')),
  percentage      numeric(5, 2) CHECK (percentage IS NULL OR (percentage > 0 AND percentage <= 100)),
  basis           text NOT NULL DEFAULT 'received' CHECK (basis IN ('received', 'remaining')),
  fixed_amount    bigint CHECK (fixed_amount IS NULL OR fixed_amount > 0),
  -- Conditions: which receipts the rule applies to.
  source          text NOT NULL DEFAULT 'any' CHECK (source IN ('any', 'invoice_payment', 'income')),
  income_type     text CHECK (income_type IS NULL OR income_type IN ('sales', 'other')),
  customer_id     uuid,
  active_from     date,
  active_to       date,
  -- Limits.
  max_amount      bigint CHECK (max_amount IS NULL OR max_amount > 0),
  cap_amount      bigint CHECK (cap_amount IS NULL OR cap_amount > 0),
  cap_period      text CHECK (cap_period IS NULL OR cap_period IN ('week', 'fortnight', 'month', 'custom')),
  cap_period_days integer CHECK (cap_period_days IS NULL OR cap_period_days BETWEEN 1 AND 366),
  cap_anchor      date,
  stop_at_target  boolean NOT NULL DEFAULT false,
  notes           text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  archived_at     timestamptz,
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, destination_id) REFERENCES allocation_destinations (business_id, id),
  FOREIGN KEY (business_id, customer_id) REFERENCES customers (business_id, id),
  CHECK ((method = 'percentage') = (percentage IS NOT NULL)),
  CHECK ((method = 'fixed') = (fixed_amount IS NOT NULL)),
  CHECK (income_type IS NULL OR source = 'income'),
  CHECK (active_to IS NULL OR active_from IS NULL OR active_to >= active_from),
  CHECK ((cap_amount IS NULL) = (cap_period IS NULL)),
  CHECK ((cap_period IS NOT DISTINCT FROM 'custom') = (cap_period_days IS NOT NULL)),
  CHECK ((cap_period IS NOT NULL AND cap_period IN ('fortnight', 'custom')) = (cap_anchor IS NOT NULL))
);
SELECT enable_tenant_isolation('allocation_rules');
CREATE INDEX allocation_rules_destination_idx ON allocation_rules (business_id, destination_id);
GRANT SELECT, INSERT, UPDATE ON allocation_rules TO solo_app;

-- ─── Allocations (one per receipt at a time) ────────────────────────────────
ALTER TABLE invoice_payments ADD CONSTRAINT invoice_payments_business_id_id_key UNIQUE (business_id, id);

CREATE TABLE allocations (
  id                 uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id        uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  source_type        text NOT NULL CHECK (source_type IN ('invoice_payment', 'income')),
  invoice_payment_id uuid,
  income_id          uuid,
  -- The receipt as it was when last synchronised (kept current while the allocation is open).
  received_date      date NOT NULL,
  received_amount    bigint NOT NULL CHECK (received_amount > 0),
  currency           char(3) NOT NULL CHECK (currency ~ '^[A-Z]{3}$'),
  -- pending: waiting for review · confirmed: entries recorded · skipped: the user chose not to
  -- allocate · reversed: entries reversed after a correction · cancelled: the receipt was deleted
  -- before confirmation.
  status             text NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'confirmed', 'skipped', 'reversed', 'cancelled')),
  confirmed_at       timestamptz,
  confirmed_by       uuid REFERENCES users (id) ON DELETE SET NULL,
  closed_at          timestamptz,
  close_reason       text CHECK (close_reason IS NULL OR length(close_reason) <= 500),
  -- The allocation this one replaced (after its receipt changed or the user reversed it).
  replaces_id        uuid,
  created_at         timestamptz NOT NULL DEFAULT now(),
  updated_at         timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, invoice_payment_id) REFERENCES invoice_payments (business_id, id),
  FOREIGN KEY (business_id, income_id) REFERENCES income_records (business_id, id),
  FOREIGN KEY (business_id, replaces_id) REFERENCES allocations (business_id, id),
  CHECK ((source_type = 'invoice_payment') = (invoice_payment_id IS NOT NULL)),
  CHECK ((source_type = 'income') = (income_id IS NOT NULL)),
  CHECK ((status = 'confirmed') = (confirmed_at IS NOT NULL) OR status = 'reversed'),
  CHECK ((status IN ('reversed', 'cancelled')) = (closed_at IS NOT NULL))
);
SELECT enable_tenant_isolation('allocations');
-- At most one open allocation per receipt.
CREATE UNIQUE INDEX allocations_payment_open_uq ON allocations (invoice_payment_id)
  WHERE status IN ('pending', 'confirmed', 'skipped');
CREATE UNIQUE INDEX allocations_income_open_uq ON allocations (income_id)
  WHERE status IN ('pending', 'confirmed', 'skipped');
CREATE INDEX allocations_status_idx ON allocations (business_id, status, received_date);
GRANT SELECT, INSERT, UPDATE ON allocations TO solo_app;

-- ─── Entries (append-only) ──────────────────────────────────────────────────
CREATE TABLE allocation_entries (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  allocation_id  uuid NOT NULL,
  destination_id uuid NOT NULL,
  -- The rule that proposed it (none when the user added or changed the amount).
  rule_id        uuid,
  kind           text NOT NULL CHECK (kind IN ('allocation', 'reversal')),
  amount         bigint NOT NULL CHECK (amount <> 0),
  reverses_id    uuid,
  created_by     uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, id),
  FOREIGN KEY (business_id, allocation_id) REFERENCES allocations (business_id, id),
  FOREIGN KEY (business_id, destination_id) REFERENCES allocation_destinations (business_id, id),
  FOREIGN KEY (business_id, rule_id) REFERENCES allocation_rules (business_id, id),
  FOREIGN KEY (business_id, reverses_id) REFERENCES allocation_entries (business_id, id),
  CHECK ((kind = 'reversal') = (reverses_id IS NOT NULL)),
  CHECK ((kind = 'allocation') = (amount > 0))
);
SELECT enable_tenant_rls('allocation_entries');
-- An entry is reversed at most once.
CREATE UNIQUE INDEX allocation_entries_reverses_uq ON allocation_entries (reverses_id) WHERE reverses_id IS NOT NULL;
CREATE INDEX allocation_entries_destination_idx ON allocation_entries (business_id, destination_id);
CREATE INDEX allocation_entries_allocation_idx ON allocation_entries (allocation_id);
GRANT SELECT, INSERT ON allocation_entries TO solo_app;
