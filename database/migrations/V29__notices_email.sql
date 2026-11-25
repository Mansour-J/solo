-- Solo — the optional weekly email of notices the owner hasn't dismissed (SL-606).

ALTER TABLE users
  ADD COLUMN notices_email boolean NOT NULL DEFAULT false,
  -- Where to send it; empty uses the send-from address in Settings › Email.
  ADD COLUMN notices_email_to text CHECK (notices_email_to IS NULL OR length(notices_email_to) BETWEEN 3 AND 320),
  ADD COLUMN notices_emailed_at timestamptz;
