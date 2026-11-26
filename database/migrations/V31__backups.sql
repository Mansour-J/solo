-- Solo — backups of the books on this computer (SL-013).
--
-- The desktop app's local server writes a backup file (the database and receipts) and restores one;
-- the API records each here, so the app can show when the books were last backed up and remind the
-- owner when that's a while ago. Not business-scoped: a backup holds every business on the computer.
-- `restored` rows record the backup a restore came from, dated when that backup was made.

CREATE TABLE backups (
  id         uuid PRIMARY KEY DEFAULT uuidv7(),
  kind       text NOT NULL CHECK (kind IN ('backup', 'restored')),
  made_at    timestamptz NOT NULL,
  size_bytes bigint NOT NULL CHECK (size_bytes >= 0),
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX backups_made_at_idx ON backups (made_at DESC);
GRANT SELECT, INSERT ON backups TO solo_app;
