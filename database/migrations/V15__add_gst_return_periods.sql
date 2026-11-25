-- Solo — how often a GST-registered business files its returns with IRD.
--
-- `gst_period`: monthly, two-monthly (the default) or six-monthly. `gst_period_end_month` is any month
-- a return period ends in; the others follow from it (with 3, two-monthly periods end in January,
-- March, May, July, September and November). Monthly returns ignore it.

ALTER TABLE businesses
  ADD COLUMN gst_period text NOT NULL DEFAULT 'two_monthly'
    CHECK (gst_period IN ('monthly', 'two_monthly', 'six_monthly')),
  ADD COLUMN gst_period_end_month smallint NOT NULL DEFAULT 3
    CHECK (gst_period_end_month BETWEEN 1 AND 12);
