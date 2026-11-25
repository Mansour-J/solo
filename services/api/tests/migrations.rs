//! The embedded migration runner (`solo_api::migrations`) against real PostgreSQL: it builds the
//! schema from nothing as Flyway does (same history, same checksums), passes the schema checks, does
//! nothing the second time, and refuses histories it can't trust. Connects as the schema owner
//! (`TEST_OWNER_DATABASE_URL`), creating and dropping scratch databases.

use solo_api::migrations::{self, MIGRATIONS};
use sqlx::{Connection, PgConnection, Row};
use url::Url;
use uuid::Uuid;

fn owner_url() -> String {
    std::env::var("TEST_OWNER_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://solo_owner:change-me-owner-password@127.0.0.1:5432/solo_test".into()
    })
}

/// A fresh, empty database owned by the test's owner role, dropped on `drop_scratch`.
async fn scratch() -> (String, PgConnection) {
    let mut admin = PgConnection::connect(&owner_url())
        .await
        .expect("owner connection (TEST_OWNER_DATABASE_URL)");
    let name = format!("solo_migrate_{}", Uuid::now_v7().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&mut admin)
        .await
        .expect("create database");
    let mut url = Url::parse(&owner_url()).expect("url");
    url.set_path(&name);
    let conn = PgConnection::connect(url.as_str())
        .await
        .expect("connect to scratch database");
    (name, conn)
}

async fn drop_scratch(name: &str, conn: PgConnection) {
    conn.close().await.expect("close");
    let mut admin = PgConnection::connect(&owner_url())
        .await
        .expect("owner connection");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {name} WITH (FORCE)"
    )))
    .execute(&mut admin)
    .await
    .expect("drop database");
}

async fn history(conn: &mut PgConnection) -> Vec<(String, String, i32)> {
    sqlx::query("SELECT version, script, checksum FROM solo.flyway_schema_history WHERE type = 'SQL' ORDER BY installed_rank")
        .fetch_all(conn)
        .await
        .expect("history")
        .iter()
        .map(|r| (r.get("version"), r.get("script"), r.get("checksum")))
        .collect()
}

#[tokio::test]
async fn an_empty_database_is_built_exactly_as_flyway_builds_it() {
    let (name, mut conn) = scratch().await;
    let latest = MIGRATIONS.last().expect("migrations").version;

    let first = migrations::migrate(&mut conn, "test")
        .await
        .expect("migrate");
    assert_eq!(
        first.applied,
        MIGRATIONS.iter().map(|m| m.version).collect::<Vec<_>>()
    );
    assert_eq!(first.current, Some(latest));
    // Nothing to do the second time.
    let second = migrations::migrate(&mut conn, "test")
        .await
        .expect("migrate again");
    assert_eq!(second.applied, Vec::<i32>::new());
    assert_eq!(second.current, Some(latest));

    // The same history Flyway recorded for the test database (migrated by Flyway in CI and locally).
    let ours = history(&mut conn).await;
    let mut flyway_db = PgConnection::connect(&owner_url())
        .await
        .expect("owner connection");
    assert_eq!(
        ours,
        history(&mut flyway_db).await,
        "versions, scripts and checksums match Flyway's"
    );

    // The result passes the same schema checks as a Flyway-built database.
    let checks = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../database/tests/schema_checks.sql"
    ))
    .expect("checks");
    sqlx::raw_sql(sqlx::AssertSqlSafe(checks))
        .execute(&mut conn)
        .await
        .expect("schema checks pass");

    drop_scratch(&name, conn).await;
}

#[tokio::test]
async fn a_database_flyway_migrated_needs_nothing() {
    let mut conn = PgConnection::connect(&owner_url())
        .await
        .expect("owner connection");
    let report = migrations::migrate(&mut conn, "test")
        .await
        .expect("migrate");
    assert!(report.applied.is_empty(), "{report:?}");
    assert_eq!(report.current, MIGRATIONS.last().map(|m| m.version));
}

#[tokio::test]
async fn histories_it_cannot_trust_are_refused() {
    let (name, mut conn) = scratch().await;
    migrations::migrate(&mut conn, "test")
        .await
        .expect("migrate");

    // A migration edited after it was applied.
    sqlx::raw_sql(
        "UPDATE solo.flyway_schema_history SET checksum = checksum + 1 WHERE version = '2'",
    )
    .execute(&mut conn)
    .await
    .expect("tamper");
    let changed = migrations::migrate(&mut conn, "test")
        .await
        .expect_err("changed migration");
    assert!(
        changed
            .to_string()
            .contains("V2__create_authentication.sql was changed"),
        "{changed}"
    );
    sqlx::raw_sql(
        "UPDATE solo.flyway_schema_history SET checksum = checksum - 1 WHERE version = '2'",
    )
    .execute(&mut conn)
    .await
    .expect("restore");

    // A database from a newer version of the app.
    sqlx::raw_sql(
        "INSERT INTO solo.flyway_schema_history (installed_rank, version, description, type, script, checksum, installed_by, execution_time, success)
         VALUES (999, '999', 'from the future', 'SQL', 'V999__from_the_future.sql', 1, 'test', 0, true)",
    )
    .execute(&mut conn)
    .await
    .expect("insert");
    let newer = migrations::migrate(&mut conn, "test")
        .await
        .expect_err("newer database");
    assert!(newer.to_string().contains("newer than this app"), "{newer}");

    // A migration that failed part-way.
    sqlx::raw_sql("UPDATE solo.flyway_schema_history SET success = false, version = '13' , script = 'V13__create_annual_accounts.sql' WHERE installed_rank = 999")
        .execute(&mut conn)
        .await
        .expect("fail");
    let failed = migrations::migrate(&mut conn, "test")
        .await
        .expect_err("failed migration");
    assert!(failed.to_string().contains("failed earlier"), "{failed}");

    drop_scratch(&name, conn).await;
}
