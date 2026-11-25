# End-to-end tests (Playwright)

- **Projects** (`playwright.config.ts`):
  - `desktop` (1440×900): every spec except `*.mobile.spec.ts`
  - `mobile` (Pixel 7): only `*.mobile.spec.ts`.
- **The config starts no server.** `baseURL` is `E2E_BASE_URL`, by default `http://localhost:5173`: Vite proxying the desktop app's local server, as in CI (`npm run local:serve`, then `VITE_DEV_API_PROXY=<its URL> npm run dev`). `/e2e` has the start-up commands.
- Run one spec: `E2E_BASE_URL=http://localhost:5173 npx playwright test e2e/<name>.spec.ts --project=desktop --retries=0 --reporter=line`.
- On failure, read `test-results/<test>/error-context.md` (accessibility snapshot) and view `test-failed-1.png` before changing anything. Traces are kept on failure.
- Specs are type-checked by `tsc -p tsconfig.e2e.json` as part of `npm run check`.

## Helpers (`helpers.ts`)

- There is no sign-in (ADR 0016): the app has one owner, shared by every test.
- `startBusinessSetup(page)` opens onboarding; `createBusiness(page, name)` completes it with only a name, which selects that business, and lands on the dashboard. `currentBusinessId(page)` returns it.
- **Isolation:** every test creates its own business, because the suite runs fully parallel against one owner. Never rely on another test's data, the business list, or the owner's name.
- **Seeding:** set up prerequisites through the API with `page.request` under `/api/v1/businesses/${await currentBusinessId(page)}`, with the `x-client-kind: web` header (`apiCustomer` in `bookkeeping.spec.ts`). The journey under test goes through the UI.

## Locator patterns that work here

- Prefer `getByRole(role, { name })` and `getByLabel`. Labels are part of the contract, and they use typographic apostrophes (`'Customer’s answer'`, `'How it’s recorded'`).
- **Mantine `Select`:** a combobox. `getByRole('combobox', { name: 'Customer' }).click()`, then `page.getByRole('option', { name })`. Options render outside the dialog, so query them on `page`.
- **Modals and drawers:** `page.getByRole('dialog', { name: 'Add expense' })`, then locate inside it. Invoice line inputs are labelled per line (`'Description, line 1'`).
- **Amounts** appear in several places (rows, totals, cards): scope to a dialog or row, or use `.first()` / `.last()`. Format is `$1,000.00`.
- **Uploads:** `locator('input[type="file"]').setInputFiles({ name, mimeType, buffer })` with real file bytes, because the API checks them (`RECEIPT_PNG` in `records.spec.ts`). Downloads: `page.waitForEvent('download')`.
- **Phone:** the navigation is behind `getByRole('button', { name: 'Toggle navigation' })`.

## Determinism

- The app's dates come from the business's today in its time zone, not the test machine's clock. Assert dates the app chose by reading them back, not by recomputing them.
- Wait for UI state (`await expect(…).toBeVisible()`), never sleep.
