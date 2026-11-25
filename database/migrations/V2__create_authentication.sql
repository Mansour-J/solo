-- Solo — authentication: users, sessions and single-use tokens (docs/architecture/authentication.md).
--
-- These tables are not business-scoped, so they have no row-level security. Only the API's auth and
-- user services touch them, always filtered by the signed-in user's ID.

-- ─── Users (one person, one login) ──────────────────────────────────────────
CREATE TABLE users (
  id                  uuid PRIMARY KEY DEFAULT uuidv7(),
  email               citext NOT NULL UNIQUE CHECK (length(email) <= 254 AND email ~ '^[^@\s]+@[^@\s]+$'),
  first_name          text NOT NULL CHECK (length(btrim(first_name)) BETWEEN 1 AND 100),
  last_name           text NOT NULL CHECK (length(btrim(last_name)) BETWEEN 1 AND 100),
  -- Argon2id PHC string. Plaintext passwords are never stored or logged.
  password_hash       text NOT NULL,
  password_changed_at timestamptz NOT NULL DEFAULT now(),
  email_verified_at   timestamptz,
  terms_accepted_at   timestamptz NOT NULL,
  terms_version       text NOT NULL,
  -- Brute-force protection: consecutive failures, and a temporary lock after too many.
  failed_login_count  integer NOT NULL DEFAULT 0 CHECK (failed_login_count >= 0),
  locked_until        timestamptz,
  last_login_at       timestamptz,
  -- UI preferences (theme, date format, number format, notifications). Validated by the API.
  preferences         jsonb NOT NULL DEFAULT '{}'::jsonb,
  created_at          timestamptz NOT NULL DEFAULT now(),
  updated_at          timestamptz NOT NULL DEFAULT now()
);
CREATE TRIGGER touch_updated_at BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
GRANT SELECT, INSERT, UPDATE, DELETE ON users TO solo_app;

-- ─── Sessions ───────────────────────────────────────────────────────────────
-- One row per signed-in device. Tokens are random 256-bit values; only their SHA-256 hashes are stored.
-- The short-lived access token authenticates requests; the refresh token rotates on every use, and
-- presenting a rotated (previous) refresh token revokes the session (token theft detection).
CREATE TABLE user_sessions (
  id                          uuid PRIMARY KEY DEFAULT uuidv7(),
  user_id                     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  client                      text NOT NULL CHECK (client IN ('web', 'desktop', 'mobile')),
  user_agent                  text NOT NULL DEFAULT '' CHECK (length(user_agent) <= 512),
  ip_address                  text CHECK (length(ip_address) <= 64),
  remember_me                 boolean NOT NULL DEFAULT false,
  access_token_hash           bytea NOT NULL UNIQUE,
  access_expires_at           timestamptz NOT NULL,
  refresh_token_hash          bytea NOT NULL UNIQUE,
  previous_refresh_token_hash bytea,
  -- When the refresh token last rotated. A previous token presented shortly afterwards is a benign race
  -- (two tabs refreshing at once), not theft, so it is rejected without revoking the session.
  refresh_rotated_at          timestamptz,
  refresh_expires_at          timestamptz NOT NULL,
  last_seen_at                timestamptz NOT NULL DEFAULT now(),
  revoked_at                  timestamptz,
  revoked_reason              text CHECK (revoked_reason IN (
                                'sign_out', 'user_revoked', 'password_changed', 'password_reset',
                                'email_changed', 'refresh_token_reuse', 'account_deleted')),
  created_at                  timestamptz NOT NULL DEFAULT now(),
  updated_at                  timestamptz NOT NULL DEFAULT now(),
  CHECK ((revoked_at IS NULL) = (revoked_reason IS NULL))
);
CREATE TRIGGER touch_updated_at BEFORE UPDATE ON user_sessions FOR EACH ROW EXECUTE FUNCTION touch_updated_at();
CREATE INDEX user_sessions_user_active_idx ON user_sessions (user_id) WHERE revoked_at IS NULL;
CREATE INDEX user_sessions_previous_refresh_idx ON user_sessions (previous_refresh_token_hash)
  WHERE previous_refresh_token_hash IS NOT NULL;
CREATE INDEX user_sessions_expiry_idx ON user_sessions (refresh_expires_at);
GRANT SELECT, INSERT, UPDATE, DELETE ON user_sessions TO solo_app;

-- ─── Single-use tokens (email verification, password reset, email change) ───
CREATE TABLE auth_tokens (
  id          uuid PRIMARY KEY DEFAULT uuidv7(),
  user_id     uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  purpose     text NOT NULL CHECK (purpose IN ('email_verification', 'password_reset', 'email_change')),
  token_hash  bytea NOT NULL UNIQUE,
  -- The address the token was sent to. For email_change this is the new, unverified address.
  email       citext NOT NULL,
  expires_at  timestamptz NOT NULL,
  consumed_at timestamptz,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX auth_tokens_user_open_idx ON auth_tokens (user_id, purpose) WHERE consumed_at IS NULL;
CREATE INDEX auth_tokens_expiry_idx ON auth_tokens (expires_at);
GRANT SELECT, INSERT, UPDATE, DELETE ON auth_tokens TO solo_app;
