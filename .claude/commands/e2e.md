---
description: Run Playwright e2e specs (desktop and/or phone) against the local server and triage failures
argument-hint: '[spec name or path] [desktop|mobile]'
---

Run Solo's end-to-end tests and triage failures. Follow `e2e/CLAUDE.md`.

- Arguments: `$ARGUMENTS`
  - A spec name like `records` means `e2e/records.spec.ts`. With no spec, run everything.
  - `desktop` or `mobile` selects `--project`; otherwise run both.
- **The suite starts nothing** (`playwright.config.ts` has no `webServer`). It needs Vite (http://localhost:5173, the default `baseURL`) proxying the desktop app's local server, as CI runs it:
  - The local server: `npm run local:serve -- --url-file data/local-url.txt` in the background (bundled PostgreSQL in `data/local`; the first build downloads it and compiles for a few minutes). It prints its URL.
  - Vite: `curl -s -o /dev/null -w '%{http_code}' http://localhost:5173/health/ready` must print 200. Otherwise start `VITE_DEV_API_PROXY=<the local server's URL> npm run dev` in the background (stop an older `npm run dev` first).
  - To test API changes, restart the local server: it compiles the API in.
- Command: `rm -rf test-results && [E2E_BASE_URL=…] npx playwright test <spec> [--project=<p>] --retries=0 --reporter=line`. Run the full suite in the background.
- For each failure:
  1. Read `test-results/<test>/error-context.md` (error plus accessibility snapshot).
  2. View `test-failed-1.png`. Traces are kept on failure (`npx playwright show-trace test-results/<test>/trace.zip`).
  3. Decide whether it is a locator or timing problem in the test, a stale server (API not restarted after a change, client not regenerated), or a real product bug. Real bugs get fixed in the product, with a regression assertion kept in the test.
  4. Re-run only the failing spec until it is green, then re-run the whole affected project once.
- Report passed and failed counts per project and what was fixed (test vs product).
