-- Solo — low-value assets (IRD: an asset costing $1,000 or less, excluding GST, can be deducted in
-- full in the year it was bought instead of being depreciated). `full` posts its whole cost as
-- depreciation in that year; the API allows it only for such assets.

ALTER TABLE fixed_assets DROP CONSTRAINT fixed_assets_method_check;
ALTER TABLE fixed_assets ADD CONSTRAINT fixed_assets_method_check CHECK (method IN ('dv', 'sl', 'full'));
