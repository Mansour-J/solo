-- Solo — email settings for the owner's own mail account (SMTP), so the desktop app can send invoices,
-- statements and reminders. Used when the server itself has no mail server configured (the desktop
-- app's local server never does). The password stays in this computer's local database.

CREATE TABLE email_settings (
  user_id      uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
  enabled      boolean NOT NULL DEFAULT true,
  host         text NOT NULL CHECK (length(btrim(host)) BETWEEN 1 AND 253),
  port         integer NOT NULL CHECK (port BETWEEN 1 AND 65535),
  -- none, starttls or tls (implicit TLS, usually port 465).
  security     text NOT NULL CHECK (security IN ('none', 'starttls', 'tls')),
  username     text NOT NULL DEFAULT '' CHECK (length(username) <= 320),
  password     text NOT NULL DEFAULT '' CHECK (length(password) <= 500),
  from_name    text NOT NULL DEFAULT '' CHECK (length(from_name) <= 120),
  from_address text NOT NULL CHECK (length(btrim(from_address)) BETWEEN 3 AND 320),
  -- The last test: when, and the error if it failed.
  tested_at    timestamptz,
  test_error   text CHECK (test_error IS NULL OR length(test_error) <= 2000),
  created_at   timestamptz NOT NULL DEFAULT now(),
  updated_at   timestamptz NOT NULL DEFAULT now()
);
GRANT SELECT, INSERT, UPDATE, DELETE ON email_settings TO solo_app;
