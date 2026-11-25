---
description: Run the Solo quality gate (client checks, API lint and tests, optionally Playwright) and report results
argument-hint: "[full]  — add 'full' to include the Playwright suite"
---

Run the project quality gate and report honestly.

1. **Test database.** `npm run api:test` needs a migrated `solo_test`. On this machine Solo's PostgreSQL runs on port 55432 (5432 is often Hermes Hub's), and there is no `.env`, so use the commands in `database/CLAUDE.md`:
   - `docker ps --format '{{.Names}} {{.Ports}}' | grep solo-db` should show `127.0.0.1:55432->5432/tcp`.
   - If it doesn't, start it: `POSTGRES_PORT=55432 docker compose --env-file environment.example up -d --wait db`.
   - If migrations were added: `POSTGRES_PORT=55432 docker compose --env-file environment.example run --rm -e FLYWAY_URL=jdbc:postgresql://db:5432/solo_test migrate`.
2. Run in this order, stopping to diagnose at the first failure:
   - `npm run check`: typecheck (client, api-client, e2e), ESLint with `--max-warnings 0`, Prettier check, Vitest.
   - `npm run api:lint`: `cargo fmt --check` and clippy with `-D warnings`.
   - `TEST_DATABASE_URL=postgres://solo_app:change-me-app-password@127.0.0.1:55432/solo_test npm run api:test`. It compiles first; run it in the background if it may take more than 2 minutes.
   - If the API changed: `npm run api:client`, then `git status --short packages/api-client`. A diff that isn't committed fails CI's `contract` job.
3. If `$ARGUMENTS` contains `full`, also run the whole Playwright suite as `/e2e` describes. It needs the local server and Vite running.
4. For each failure, find the root cause: read the error, open the test and the code under test, and for Playwright read `test-results/*/error-context.md` and the screenshot. Say whether it is a test problem or a product bug. Never weaken an assertion, skip a test or loosen a lint rule to make the gate pass.
5. Report a short table (check → result with counts), then failures with cause and the proposed or applied fix. Don't claim a check passed unless you ran it.
