# Local server (`solo-local`)

The desktop app's own server (ADR 0014): a bundled PostgreSQL, the schema migrations and the API in one
process, on 127.0.0.1 only. The Tauri shell starts it (`apps/client/src-tauri/src/backend.rs`); the
`solo-local` binary runs it on its own for development and end-to-end tests.

- `LocalServer::start(Options { data_dir, cors_allowed_origins, require_key })` unpacks PostgreSQL into
  `<data_dir>/postgresql` on first start and keeps the cluster in `<data_dir>/database`, files in
  `<data_dir>/storage` and the owner's password in `<data_dir>/local.json` (owner-only). The API's
  role `solo_app` gets a new random password on each start. `stop()` stops the API, then
  PostgreSQL (a fast shutdown). A database still running from a killed app is stopped and restarted.
- The API runs with `EMAIL_PROVIDER=log` (no mail server) as the owner (`ensure_owner`, created on first start). With `require_key` (the desktop app) it requires a new random `x-api-key` on each start, given by `key()`; the `solo-local` binary runs without one for the Vite dev server and Playwright.
- PostgreSQL is bundled at build time (`postgresql_embedded`, feature `bundled`). The version is pinned
  by `POSTGRESQL_VERSION` in the repository's `.cargo/config.toml` and must match `POSTGRES_VERSION`
  in `src/lib.rs`. The first build downloads the archive from GitHub (set `GITHUB_TOKEN` to avoid
  rate limits).

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path services/local/Cargo.toml   # first start, restart, recovery after a kill (~20 s)
npm run local:serve                                    # data in ./data/local; prints the API URL
```
