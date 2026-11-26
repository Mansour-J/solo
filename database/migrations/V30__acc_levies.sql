-- Solo — ACC levies for self-employed people (SL-801), estimated on the Income tax page.
--
-- Reference data, not business-scoped: one row per ACC levy year (1 April – 31 March), applying to
-- income years starting on or after `effective_from` until a later row. Rates are dollars per $100 of
-- liable earnings, excluding GST (ACC adds GST to self-employed invoices). Liable earnings below
-- `min_liable` are levied at the minimum for someone working full-time (over 30 hours a week on
-- average); earnings above `max_liable` aren't levied.
--
-- Sources (checked 26 November 2026): the earners' levy from the Government's decision on the
-- 2025/26 – 2027/28 ACC levies (MBIE; $1.39 in 2024/25, then $1.45, $1.52 and $1.59 per $100 excl.
-- GST; IRD shows the same rates including GST: $1.60, $1.67, $1.75); the minimum and maximum
-- liable earnings and the Working Safer levy ($0.08 per $100) from acc.co.nz "Calculating your
-- levies". The work levy depends on the business's ACC classification unit, so the owner enters it
-- (business_settings.acc_work_levy_rate).

CREATE TABLE acc_levy_rates (
  effective_from     date PRIMARY KEY CHECK (extract(month FROM effective_from) = 4 AND extract(day FROM effective_from) = 1),
  earners_levy       numeric(6, 4) NOT NULL CHECK (earners_levy >= 0 AND earners_levy < 100),
  working_safer_levy numeric(6, 4) NOT NULL CHECK (working_safer_levy >= 0 AND working_safer_levy < 100),
  min_liable         bigint NOT NULL CHECK (min_liable >= 0),
  max_liable         bigint NOT NULL CHECK (max_liable > min_liable),
  created_at         timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT ON acc_levy_rates TO solo_app;

INSERT INTO acc_levy_rates (effective_from, earners_levy, working_safer_levy, min_liable, max_liable) VALUES
  (DATE '2024-04-01', 1.39, 0.08, 4425000, 14228300),
  (DATE '2025-04-01', 1.45, 0.08, 4936500, 15279000),
  (DATE '2026-04-01', 1.52, 0.08, 5050100, 15664100);

-- The owner's work levy rate (dollars per $100, excl. GST, from their ACC invoice or ACC's levy
-- guidebook for their classification unit) and whether they work full-time for the minimum.
ALTER TABLE business_settings
  ADD COLUMN acc_work_levy_rate numeric(6, 4) CHECK (acc_work_levy_rate IS NULL OR (acc_work_levy_rate >= 0 AND acc_work_levy_rate < 100)),
  ADD COLUMN acc_full_time boolean NOT NULL DEFAULT false;
