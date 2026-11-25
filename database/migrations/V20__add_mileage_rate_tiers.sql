-- Solo — IRD's two-tier kilometre rates: a tier 1 rate for the first 14,000 km of a vehicle's travel in
-- the year and a tier 2 rate after it. The tiers are data on each rate version (both or neither), and a
-- trip keeps the tiers it was priced with, like its rate.

ALTER TABLE mileage_rate_versions
  ADD COLUMN tier1_limit_km    numeric(8, 1) CHECK (tier1_limit_km > 0),
  ADD COLUMN tier2_rate_per_km numeric(10, 4) CHECK (tier2_rate_per_km >= 0 AND tier2_rate_per_km <= 100),
  ADD CONSTRAINT mileage_rate_versions_tiers_check CHECK ((tier1_limit_km IS NULL) = (tier2_rate_per_km IS NULL));

ALTER TABLE mileage_trips
  ADD COLUMN tier1_limit_km    numeric(8, 1) CHECK (tier1_limit_km > 0),
  ADD COLUMN tier2_rate_per_km numeric(10, 4) CHECK (tier2_rate_per_km >= 0),
  -- Of business_distance_km, the kilometres at the tier 2 rate.
  ADD COLUMN tier2_distance_km numeric(8, 1) NOT NULL DEFAULT 0 CHECK (tier2_distance_km >= 0),
  ADD CONSTRAINT mileage_trips_tiers_check CHECK ((tier1_limit_km IS NULL) = (tier2_rate_per_km IS NULL)),
  ADD CONSTRAINT mileage_trips_tier2_distance_check CHECK (tier2_distance_km <= business_distance_km);
