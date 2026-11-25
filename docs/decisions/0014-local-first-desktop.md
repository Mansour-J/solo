# 0014 — Local-first desktop app: embedded PostgreSQL by default, a cloud server as an option

- **Status:** Accepted. The cloud-server option and accounts were removed by 0016.
- **Date:** 2026-11-21

## Context

Using Solo meant running a server: Docker with PostgreSQL, the API, the web app and Flyway (Java).
The desktop app was only a window onto that API. For a sole trader, "install and go" matters more
than a shared server, but the schema relies on PostgreSQL throughout: row-level security for tenant
isolation, PL/pgSQL functions (the chart of accounts, triggers), a deferred constraint that keeps
every journal entry balanced, and PostgreSQL-only SQL (`FILTER`, `ILIKE`, `generate_series`, `jsonb`).

Options considered: SQLite (a second dialect, no row-level security or PL/pgSQL), PGlite (PostgreSQL
in WebAssembly for JavaScript; our API is Rust, so it would need a Node sidecar and a socket bridge),
and embedding real PostgreSQL.

## Decision

- **The desktop app keeps the books on the user's computer by default.** It runs the local server
  (`services/local`, crate `solo-local`) in its own process:
  - PostgreSQL 18, bundled into the binary at build time (`postgresql_embedded`, version pinned in
    `.cargo/config.toml` and `solo_local::POSTGRES_VERSION`), unpacked on first start into the
    local (not roaming) app-data folder, with the cluster beside it. It listens on 127.0.0.1 on a
    random port, with password authentication; the owner's password is created on first start and
    kept in an owner-only file, and the API's role gets a new random password on every start.
  - The schema migrations, applied by `solo_api::migrations`: the same files, the same
    `flyway_schema_history` table and the same checksums as Flyway, so either can manage a database
    the other built. Edited, failed or newer histories are refused. `solo-api migrate` exposes it.
  - The API, on 127.0.0.1 on a random port, with `DEPLOYMENT=local`.
- **The same code path everywhere.** It's the same PostgreSQL, schema and API as the Docker and
  cloud deployments, so tenant isolation, the balance constraint and every report behave the same.
- **A cloud server is the option.** The user chooses in Settings › Where your books are (or from the
  start-up screen if the local server can't start): this computer, or a cloud server's HTTPS address.
  The choice is saved in `connection.json`, and switching restarts the app, so a session never spans
  two servers. Refresh tokens are kept per kind of server.
- **A local server has no mail server** (`EMAIL_PROVIDER=log`): new accounts count as verified;
  `GET /api/v1/meta` reports `deployment` and `email_enabled`, and the client hides sending
  documents, password resets and email changes when email is off. Sending a document is refused
  with an explanation rather than pretending to send.
- **Lifecycle:** the app stops the API and then PostgreSQL (a fast shutdown) when it exits. If the
  app was killed, the next start finds the database still running on its folder, stops it cleanly and
  starts again.

## Consequences

- Installing is downloading the app. Installers grow by the PostgreSQL archive (about 50 MB on
  Windows, about 90 MB per architecture on macOS) and first start takes a few seconds.
- A major PostgreSQL upgrade will need a `pg_upgrade` step before the pinned version changes.
- Local books are only as safe as the computer: the export in Settings › Data & privacy is the copy
  for now; a backup and restore action is a follow-up (backlog), as are moving books between this
  computer and a cloud server, SMTP settings for local mode and password recovery without email.
- The desktop shell now owns a database process; `src-tauri` is no longer a thin window only.

## Alternatives considered

- **SQLite:** rewrite every migration and many queries, move isolation and the balance check into
  Rust, and maintain two dialects or abandon PostgreSQL. Rejected: most risk to the bookkeeping that
  is proven against annual accounts figure by figure.
- **PGlite:** JavaScript-first, single connection, a second runtime. Rejected for a Rust API.
- **Cloud only:** simplest for users, but an operations and privacy commitment; kept as the option.
