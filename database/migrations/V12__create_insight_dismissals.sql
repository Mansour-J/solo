-- Solo — insights: notices a user has dismissed. Insights themselves are calculated on request from
-- the journal and the records (nothing is stored); only a person's "dismiss" is kept, per business.
-- A notice's key includes what it says (e.g. the count and total of overdue invoices), so a dismissed
-- notice comes back when the facts behind it change (ADR 0012).

CREATE TABLE insight_dismissals (
  id           uuid PRIMARY KEY DEFAULT uuidv7(),
  business_id  uuid NOT NULL REFERENCES businesses (id) ON DELETE CASCADE,
  user_id      uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  notice_key   text NOT NULL CHECK (length(notice_key) BETWEEN 1 AND 200),
  dismissed_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (business_id, user_id, notice_key)
);
SELECT enable_tenant_rls('insight_dismissals');
GRANT SELECT, INSERT, DELETE ON insight_dismissals TO solo_app;
