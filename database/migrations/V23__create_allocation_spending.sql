-- Solo — spending from an allocation: money set aside that was then used (a GST payment to IRD, a
-- holiday from savings). It lowers the destination's balance. Recorded, never paid by Solo.

CREATE TABLE allocation_spending (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id    uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  destination_id uuid NOT NULL,
  spent_on       date NOT NULL,
  amount         bigint NOT NULL CHECK (amount > 0),
  description    text NOT NULL CHECK (length(btrim(description)) BETWEEN 1 AND 200),
  notes          text NOT NULL DEFAULT '' CHECK (length(notes) <= 1000),
  voided_at      timestamptz,
  void_reason    text CHECK (void_reason IS NULL OR length(void_reason) <= 500),
  created_by     uuid REFERENCES users (id) ON DELETE SET NULL,
  created_at     timestamptz NOT NULL DEFAULT now(),
  updated_at     timestamptz NOT NULL DEFAULT now(),
  FOREIGN KEY (business_id, destination_id) REFERENCES allocation_destinations (business_id, id),
  UNIQUE (business_id, id)
);
SELECT enable_tenant_isolation('allocation_spending');
CREATE INDEX allocation_spending_destination_idx ON allocation_spending (business_id, destination_id) WHERE voided_at IS NULL;
GRANT SELECT, INSERT, UPDATE ON allocation_spending TO solo_app;
