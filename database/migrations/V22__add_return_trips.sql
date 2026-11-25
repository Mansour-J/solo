-- Solo — return trips: the distance entered is one way and the trip counts it there and back.
-- `distance_km` stays the whole trip, so totals, claims and tiers are unchanged.

ALTER TABLE mileage_trips
  ADD COLUMN return_trip boolean NOT NULL DEFAULT false,
  ADD CONSTRAINT mileage_trips_return_trip_check CHECK (NOT return_trip OR start_odometer IS NULL);
