-- Solo — versioned tax configuration (reference data, not business-scoped).
--
-- Tax rates are data with effective dates, never constants in code. The API resolves the rate that
-- applies on a document's date. effective_to is exclusive; NULL means "still in effect".

CREATE TABLE tax_rates (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  jurisdiction   char(2) NOT NULL CHECK (jurisdiction ~ '^[A-Z]{2}$'),
  tax_type       text NOT NULL CHECK (tax_type IN ('gst')),
  rate           numeric(7, 6) NOT NULL CHECK (rate >= 0 AND rate < 1),
  effective_from date NOT NULL,
  effective_to   date,
  description    text NOT NULL DEFAULT '',
  created_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (jurisdiction, tax_type, effective_from),
  CHECK (effective_to IS NULL OR effective_to > effective_from)
);
-- No two versions of the same tax may overlap.
CREATE EXTENSION IF NOT EXISTS btree_gist;
ALTER TABLE tax_rates ADD CONSTRAINT tax_rates_no_overlap
  EXCLUDE USING gist (jurisdiction WITH =, tax_type WITH =, daterange(effective_from, effective_to) WITH &&);
GRANT SELECT ON tax_rates TO solo_app;

-- New Zealand GST: 12.5% from 1 July 1989, 15% from 1 October 2010.
INSERT INTO tax_rates (jurisdiction, tax_type, rate, effective_from, effective_to, description) VALUES
  ('NZ', 'gst', 0.125, DATE '1989-07-01', DATE '2010-10-01', 'GST 12.5%'),
  ('NZ', 'gst', 0.150, DATE '2010-10-01', NULL, 'GST 15%');
