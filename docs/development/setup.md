# Development setup

## Prerequisites

| For | Requirement |
|---|---|
| Web client, tooling | Node.js 24 and npm |
| API | Rust stable (`rustup`), with `rustfmt` and `clippy` components |
| The API's integration tests | Docker with Compose v2 (PostgreSQL) |
| Desktop app | Tauri 2 prerequisites. Debian/Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev` |

## First run

The quickest way to see the app is the desktop app itself, which starts its own database and API:

```bash
npm install
npm run desktop:dev                  # a native window on the Vite dev server
npm run desktop:build                # installers for this OS in apps/client/src-tauri/target/release/bundle
```

On Linux the app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` itself (a blank window otherwise).

## In a browser

```bash
npm run local:serve                  # the local server (bundled PostgreSQL + API), data in ./data/local; prints its URL
VITE_DEV_API_PROXY=<that URL> npm run dev   # the app on http://localhost:5173
```

There is no sign-in: the app opens on onboarding, then the dashboard. Restart `local:serve` after
changing the API.

## The development database (integration tests)

```bash
cp environment.example .env
npm run db:up                        # PostgreSQL 18 on localhost:${POSTGRES_PORT:-5432}
npm run db:migrate                   # apply migrations to the development database
npm run db:test:prepare              # migrate the integration-test database (solo_test)
npm run api:dev                      # optional: the API on http://localhost:8000 against it
```

If port 5432 is already used (another project's database), set `POSTGRES_PORT=55432` in `.env`.
`docker compose up -d mailpit` catches the invoices `api:dev` emails at http://localhost:8025.

## Changing the API

1. Domain rules in `services/api/src/domain` (pure, unit-tested).
2. SQL in `repositories/`, logic and transactions in `services/`, a thin handler with a
   `#[utoipa::path]` annotation in `api/`, registered in the module's `router()`.
3. Integration tests in `services/api/tests` (real PostgreSQL), including a cross-tenant test.
4. `npm run api:client` to regenerate the TypeScript client, then use it from the feature's `api.ts`.

The OpenAPI document is browsable at `/api/docs` on the API.

## Adding a module to the client

`apps/client/src/features/<module>/` with `api.ts` (query keys and hooks), form schemas, pages and
components; register routes in `src/app/router.tsx` and, once it works end to end, add it to
`src/app/nav.ts`.
