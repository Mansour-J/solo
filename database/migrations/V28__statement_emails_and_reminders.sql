-- Solo — statement emails (SL-513) and invoice reminders (SL-702).

-- A statement email is logged against the customer it's for.
ALTER TABLE email_logs DROP CONSTRAINT email_logs_document_type_check;
ALTER TABLE email_logs ADD CONSTRAINT email_logs_document_type_check
  CHECK (document_type IN ('invoice', 'quote', 'statement'));

-- A reminder is an invoice email with the day it was for, relative to the due date (-3 = three days
-- before, 7 = a week after). Each is sent once per invoice.
ALTER TABLE email_logs ADD COLUMN reminder_day integer CHECK (reminder_day IS NULL OR reminder_day BETWEEN -60 AND 365);
CREATE UNIQUE INDEX email_logs_reminder_once ON email_logs (document_id, reminder_day)
  WHERE reminder_day IS NOT NULL AND status = 'sent';

-- Reminders are off until the owner turns them on.
ALTER TABLE business_settings
  ADD COLUMN reminders_enabled boolean NOT NULL DEFAULT false,
  ADD COLUMN reminder_days_before integer CHECK (reminder_days_before IS NULL OR reminder_days_before BETWEEN 1 AND 60),
  ADD COLUMN reminder_days_after integer[] NOT NULL DEFAULT '{1,7,14}'
    CHECK (cardinality(reminder_days_after) <= 6 AND 0 < ALL (reminder_days_after) AND 366 > ALL (reminder_days_after));
UPDATE business_settings SET reminder_days_before = 3;

-- A customer can be left out of reminders.
ALTER TABLE customers ADD COLUMN send_reminders boolean NOT NULL DEFAULT true;
