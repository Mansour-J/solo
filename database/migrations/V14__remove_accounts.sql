-- Solo — no accounts: the desktop app is for one person, so there is no sign-in (ADR 0016).
--
-- The `users` table stays as the owner's profile (name and preferences): businesses, members and
-- the audit log refer to it, and row-level security still scopes every query to it. Sessions,
-- single-use tokens, passwords and email addresses go.

DROP TABLE auth_tokens;
DROP TABLE user_sessions;

ALTER TABLE users
  DROP COLUMN email,
  DROP COLUMN password_hash,
  DROP COLUMN password_changed_at,
  DROP COLUMN email_verified_at,
  DROP COLUMN terms_accepted_at,
  DROP COLUMN terms_version,
  DROP COLUMN failed_login_count,
  DROP COLUMN locked_until,
  DROP COLUMN last_login_at;

-- The owner is created on first start, before anyone has typed a name.
ALTER TABLE users DROP CONSTRAINT users_first_name_check;
ALTER TABLE users DROP CONSTRAINT users_last_name_check;
ALTER TABLE users
  ALTER COLUMN first_name SET DEFAULT '',
  ALTER COLUMN last_name SET DEFAULT '',
  ADD CONSTRAINT users_first_name_check CHECK (length(first_name) <= 100),
  ADD CONSTRAINT users_last_name_check CHECK (length(last_name) <= 100);
