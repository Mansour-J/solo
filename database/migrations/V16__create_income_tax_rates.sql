-- Solo — New Zealand individual income tax rates (reference data, not business-scoped).
--
-- One row per band: income from `threshold` (inclusive, in cents) up to the next band's threshold is
-- taxed at `rate`. A set of bands applies to tax years starting on or after `effective_from`, until a
-- later set. Source: ird.govt.nz "Tax rates for individuals" (checked 24 November 2026).

CREATE TABLE income_tax_rates (
  id             uuid PRIMARY KEY DEFAULT uuidv7(),
  jurisdiction   char(2) NOT NULL CHECK (jurisdiction ~ '^[A-Z]{2}$'),
  effective_from date NOT NULL,
  threshold      bigint NOT NULL CHECK (threshold >= 0),
  rate           numeric(7, 6) NOT NULL CHECK (rate >= 0 AND rate < 1),
  description    text NOT NULL DEFAULT '',
  created_at     timestamptz NOT NULL DEFAULT now(),
  UNIQUE (jurisdiction, effective_from, threshold)
);
GRANT SELECT ON income_tax_rates TO solo_app;

INSERT INTO income_tax_rates (jurisdiction, effective_from, threshold, rate, description) VALUES
  -- From 1 April 2021 (the 39% rate) until the 2024-25 tax year.
  ('NZ', DATE '2021-04-01', 0,          0.105,  '0 – $14,000'),
  ('NZ', DATE '2021-04-01', 1400000,    0.175,  '$14,001 – $48,000'),
  ('NZ', DATE '2021-04-01', 4800000,    0.30,   '$48,001 – $70,000'),
  ('NZ', DATE '2021-04-01', 7000000,    0.33,   '$70,001 – $180,000'),
  ('NZ', DATE '2021-04-01', 18000000,   0.39,   '$180,001 and over'),
  -- 2024-25: composite rates for the threshold change on 31 July 2024.
  ('NZ', DATE '2024-04-01', 0,          0.105,  '0 – $14,000'),
  ('NZ', DATE '2024-04-01', 1400000,    0.1282, '$14,001 – $15,600'),
  ('NZ', DATE '2024-04-01', 1560000,    0.175,  '$15,601 – $48,000'),
  ('NZ', DATE '2024-04-01', 4800000,    0.2164, '$48,001 – $53,500'),
  ('NZ', DATE '2024-04-01', 5350000,    0.30,   '$53,501 – $70,000'),
  ('NZ', DATE '2024-04-01', 7000000,    0.3099, '$70,001 – $78,100'),
  ('NZ', DATE '2024-04-01', 7810000,    0.33,   '$78,101 – $180,000'),
  ('NZ', DATE '2024-04-01', 18000000,   0.39,   '$180,001 and over'),
  -- From 1 April 2025.
  ('NZ', DATE '2025-04-01', 0,          0.105,  '0 – $15,600'),
  ('NZ', DATE '2025-04-01', 1560000,    0.175,  '$15,601 – $53,500'),
  ('NZ', DATE '2025-04-01', 5350000,    0.30,   '$53,501 – $78,100'),
  ('NZ', DATE '2025-04-01', 7810000,    0.33,   '$78,101 – $180,000'),
  ('NZ', DATE '2025-04-01', 18000000,   0.39,   '$180,001 and over');
