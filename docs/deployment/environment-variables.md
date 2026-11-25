# Environment variables

For development only: the desktop app configures its own local server and needs none of these. Set
them in `.env` at the repository root (template: `environment.example`) for the development database,
Mailpit and `scripts/api-dev.mjs`. Only `VITE_*` values reach client bundles, so they must never be
secrets.

## General and branding

| Variable | Default | Purpose |
|---|---|---|
| `APP_ENV` | `development` | `production` refuses test-only backends (memory email/storage) |
| `APP_URL` | `http://localhost:8080` | The app's address, used for links in emails |
| `APP_NAME`, `APP_SHORT_NAME`, `APP_DESCRIPTION`, `APP_TAGLINE` | from `brand.json` | Product identity ([branding](../development/branding.md)) |
| `APP_PRIMARY_COLOR`, `APP_SECONDARY_COLOR` | from `brand.json` | `#rrggbb` theme colours |
| `APP_SUPPORT_EMAIL`, `APP_WEBSITE_URL`, `APP_COPYRIGHT` | from `brand.json` | Shown in the app and emails |
| `APP_EMAIL_SENDER_NAME`, `APP_EMAIL_FROM` | from `brand.json` | Email "From" |
| `APP_TERMS_URL`, `APP_PRIVACY_URL` | empty | Brand links |

## Ports

| Variable | Default | Purpose |
|---|---|---|
| `POSTGRES_PORT` | `5432` | PostgreSQL on `127.0.0.1` |
| `MAILPIT_UI_PORT`, `MAILPIT_SMTP_PORT` | `8025`, `1025` | Mailpit |

## Database

| Variable | Default | Purpose |
|---|---|---|
| `POSTGRES_DB` | `solo` | Database name (`<name>_test` is created for tests) |
| `POSTGRES_USER` / `POSTGRES_PASSWORD` | `solo_owner` / required | Owner: migrations and backups only |
| `APP_DB_PASSWORD` | required | Password of the API role `solo_app` |
| `DATABASE_URL` | built by Compose | API connection string (as `solo_app`) when running outside Compose |
| `DATABASE_MAX_CONNECTIONS` | `10` | API pool size |
| `MIGRATE_DATABASE_URL` | none | Owner connection string for `solo-api migrate`, which applies the embedded migrations as Flyway would |
| `TEST_OWNER_DATABASE_URL` | `postgres://solo_owner:change-me-owner-password@127.0.0.1:5432/solo_test` | Tests only: the owner connection the migration tests use to create scratch databases |

## Email

| Variable | Default | Purpose |
|---|---|---|
| `EMAIL_PROVIDER` | `smtp` | `smtp`, or `log` (write emails to the log instead of sending) |
| `SMTP_HOST`, `SMTP_PORT` | `mailpit`, `1025` | SMTP relay |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | empty | Credentials |
| `SMTP_TLS` | `none` | `none`, `starttls` or `tls` |

## Storage

| Variable | Default | Purpose |
|---|---|---|
| `STORAGE_BACKEND` | `local` | `local` or `s3` |
| `STORAGE_LOCAL_PATH` | `./data/storage` | Local storage directory |
| `S3_BUCKET`, `S3_REGION`, `S3_ENDPOINT` | — | S3-compatible storage |
| `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY` | — | Credentials (or use the platform's instance role) |
| `S3_FORCE_PATH_STYLE` | `false` | `true` for MinIO and some providers |

## HTTP and logging

| Variable | Default | Purpose |
|---|---|---|
| `CORS_ALLOWED_ORIGINS` | `tauri://localhost,http://tauri.localhost` | Extra origins allowed to call the API |
| `API_KEY` | none | When set, every `/api` request must send it as `x-api-key`. The desktop app's local server sets a random one on each start (ADR 0016) |
| `TRUST_PROXY` | `false` | Take the client IP for the audit log from `X-Real-IP` |
| `API_MAX_UPLOAD_MB` | `10` | Maximum request body |
| `API_REQUEST_TIMEOUT_SECONDS` | `30` | Request timeout |
| `LOG_FORMAT` | `json` | `json` or `pretty` |
| `RUST_LOG` | `info,sqlx=warn` | Log filter |

## Client build

| Variable | Default | Purpose |
|---|---|---|
| `VITE_API_BASE_URL` | `http://localhost:8000` | The API `npm run desktop:dev` uses before its local server is ready. The web build uses its own origin |
| `VITE_DEV_API_PROXY` | `http://localhost:8000` | Where the Vite dev server proxies `/api` |
