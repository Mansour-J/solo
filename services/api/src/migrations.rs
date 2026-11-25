//! Applies the embedded schema migrations (`database/migrations`) the way Flyway does, recording them
//! in the same `flyway_schema_history` table with the same checksums. A database migrated here can be
//! taken over by Flyway and the other way round. Used where Flyway isn't available: the desktop
//! app's local database and `solo-api migrate`.
//!
//! Each pending migration runs in its own transaction as the connected (owner) role, with
//! `search_path` set to the `solo` schema. Concurrent runs are serialised with an advisory lock.

use std::time::Instant;

use anyhow::{Context, bail};
use sqlx::{Connection, PgConnection, Row};

/// One migration file, embedded at build time.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub script: &'static str,
    pub sql: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

/// The highest migration version embedded in this build.
pub fn latest_version() -> i32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

/// An arbitrary key for `pg_advisory_lock`, so two starts can't migrate at once.
const LOCK_KEY: i64 = 0x50_4c_4f_5f_6d_69_67;

const CREATE_HISTORY: &str = r#"
CREATE TABLE IF NOT EXISTS solo.flyway_schema_history (
  installed_rank integer NOT NULL,
  version        varchar(50),
  description    varchar(200) NOT NULL,
  type           varchar(20) NOT NULL,
  script         varchar(1000) NOT NULL,
  checksum       integer,
  installed_by   varchar(100) NOT NULL,
  installed_on   timestamp NOT NULL DEFAULT now(),
  execution_time integer NOT NULL,
  success        boolean NOT NULL,
  CONSTRAINT flyway_schema_history_pk PRIMARY KEY (installed_rank)
);
CREATE INDEX IF NOT EXISTS flyway_schema_history_s_idx ON solo.flyway_schema_history (success);
"#;

/// Flyway's checksum: CRC-32 over the UTF-8 bytes of each line, without line endings or a BOM.
pub fn checksum(sql: &str) -> i32 {
    let sql = sql.strip_prefix('\u{feff}').unwrap_or(sql);
    let mut crc = !0u32;
    let mut update = |bytes: &[u8]| {
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
    };
    for line in sql.split('\n') {
        update(line.strip_suffix('\r').unwrap_or(line).as_bytes());
    }
    // `split` yields a final empty piece after a trailing newline; Flyway reads no such line, and an
    // empty line adds nothing to the CRC either way.
    (!crc).cast_signed()
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Versions applied by this run, in order.
    pub applied: Vec<i32>,
    /// The schema version afterwards.
    pub current: Option<i32>,
}

/// Brings the database up to date. The connection must be the schema owner (it creates tables).
pub async fn migrate(conn: &mut PgConnection, installed_by: &str) -> anyhow::Result<Report> {
    run(conn, installed_by, MIGRATIONS).await
}

async fn run(
    conn: &mut PgConnection,
    installed_by: &str,
    migrations: &[Migration],
) -> anyhow::Result<Report> {
    sqlx::raw_sql("CREATE SCHEMA IF NOT EXISTS solo")
        .execute(&mut *conn)
        .await
        .context("creating the schema")?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(LOCK_KEY)
        .execute(&mut *conn)
        .await?;
    let result = apply(conn, installed_by, migrations).await;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(LOCK_KEY)
        .execute(&mut *conn)
        .await?;
    result
}

async fn apply(
    conn: &mut PgConnection,
    installed_by: &str,
    migrations: &[Migration],
) -> anyhow::Result<Report> {
    sqlx::raw_sql(CREATE_HISTORY)
        .execute(&mut *conn)
        .await
        .context("creating the migration history")?;
    let rows = sqlx::query(
        "SELECT installed_rank, version, script, checksum, success FROM solo.flyway_schema_history
         WHERE type = 'SQL' ORDER BY installed_rank",
    )
    .fetch_all(&mut *conn)
    .await?;
    let mut current = None;
    for row in &rows {
        let script: String = row.get("script");
        if !row.get::<bool, _>("success") {
            bail!("migration {script} failed earlier; repair the database before starting again");
        }
        let version: Option<String> = row.get("version");
        let version: i32 = version
            .as_deref()
            .unwrap_or_default()
            .parse()
            .with_context(|| format!("unexpected version for {script}"))?;
        let Some(known) = migrations.iter().find(|m| m.version == version) else {
            bail!(
                "the database is at schema version {version}, newer than this app understands: update the app"
            );
        };
        let recorded: Option<i32> = row.get("checksum");
        if known.script != script || recorded != Some(checksum(known.sql)) {
            bail!(
                "migration {} was changed after it was applied (recorded {script}, checksum {recorded:?})",
                known.script
            );
        }
        current = Some(current.unwrap_or(version).max(version));
    }
    let mut rank: i32 = sqlx::query_scalar(
        "SELECT coalesce(max(installed_rank), 0) FROM solo.flyway_schema_history",
    )
    .fetch_one(&mut *conn)
    .await?;
    let mut applied = Vec::new();
    let start = current;
    for migration in migrations
        .iter()
        .filter(|m| start.is_none_or(|c| m.version > c))
    {
        let started = Instant::now();
        let mut tx = conn.begin().await?;
        sqlx::raw_sql("SET LOCAL search_path = solo")
            .execute(&mut *tx)
            .await?;
        sqlx::raw_sql(migration.sql)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("applying {}", migration.script))?;
        rank += 1;
        let elapsed = i32::try_from(started.elapsed().as_millis()).unwrap_or(i32::MAX);
        sqlx::query(
            "INSERT INTO solo.flyway_schema_history
               (installed_rank, version, description, type, script, checksum, installed_by, execution_time, success)
             VALUES ($1, $2, $3, 'SQL', $4, $5, $6, $7, true)",
        )
        .bind(rank)
        .bind(migration.version.to_string())
        .bind(migration.description)
        .bind(migration.script)
        .bind(checksum(migration.sql))
        .bind(installed_by)
        .bind(elapsed)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(script = migration.script, ms = elapsed, "migration applied");
        applied.push(migration.version);
        current = Some(migration.version);
    }
    Ok(Report { applied, current })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_flyway() {
        // Recorded by Flyway 11 for the first and latest migrations of this repository.
        let first = MIGRATIONS.iter().find(|m| m.version == 1).expect("V1");
        assert_eq!(checksum(first.sql), 786_440_039);
        let v13 = MIGRATIONS.iter().find(|m| m.version == 13).expect("V13");
        assert_eq!(checksum(v13.sql), -255_050_116);
        // Line endings and a byte-order mark don't change the checksum.
        assert_eq!(checksum("a\nb\n"), checksum("\u{feff}a\r\nb\r\n"));
        assert_eq!(checksum("a\nb"), checksum("a\nb\n"));
    }

    #[test]
    fn migrations_are_embedded_in_order_with_flyway_descriptions() {
        assert!(MIGRATIONS.len() >= 13);
        assert!(MIGRATIONS.windows(2).all(|w| w[0].version < w[1].version));
        let v8 = MIGRATIONS.iter().find(|m| m.version == 8).expect("V8");
        assert_eq!(v8.description, "create expenses and income");
        assert_eq!(v8.script, "V8__create_expenses_and_income.sql");
        assert!(v8.sql.contains("CREATE TABLE"));
    }
}
