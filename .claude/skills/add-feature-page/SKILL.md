---
name: add-feature-page
description: Create or extend a Solo client screen — feature folder with API hooks and form schema, route and nav entry (only when it works end to end), useApiMutation with invalidation, MoneyInput and server-calculated totals, Mantine useForm with zod, ?new=1 deep links, loading/empty/error states, a distinct phone layout, component tests and a Playwright journey. Use when adding a page, modal, drawer or new module under apps/client/src/features.
---

# Add a feature page

Read `apps/client/CLAUDE.md` first. Use these as references:

- `features/income`: a list page with a form modal
- `features/expenses`: the same, plus a detail drawer (`?expense=<id>`) and attachments
- `features/customers`: a list and a detail route.

All paths below are under `apps/client/src/`.

## 1. Data first

List the endpoints the screen needs. If any are missing, follow the `add-api-endpoint` skill and run `npm run api:client` before building UI. Types come from `@solo/api-client` (`components['schemas']['…']`).

## 2. Feature folder

```
features/<module>/
  api.ts             query keys + useQuery hooks
  <thing>Form.ts     form values, zod schema (mirrors the API's rules), toInput/fromRow conversion, option lists
  <Things>Page.tsx   the page
  <Thing>FormModal.tsx
  routes.tsx         export const <module>Routes: RouteObject[]
  <module>.test.tsx
```

- Component files export only components (Fast Refresh lint). Hooks, schemas and helpers go in `.ts` files.
- **Query keys** live under the business: `['business', businessId, '<module>', …]` (`incomeKeys` in `features/income/api.ts`). Shared keys are `businessKeys` in `features/businesses/api.ts`.
- **Reads:** `useQuery` + `unwrap(api.GET('/api/v1/businesses/{business_id}/…', { params: { path: { business_id }, query } }))`. Lists use `placeholderData: (previous) => previous` so paging doesn't flash.

## 3. Route and navigation

- Add the routes to the `AppLayout` children in `app/router.tsx` (`...<module>Routes`).
- Add a `NavItem` (`key`, `label`, `path`, lucide `icon`) to `NAV_ITEMS` in `app/nav.ts` **only once the module works end to end**, in the documented order. The sidebar and the phone navigation drawer both read it. No placeholder pages, "coming soon" or disabled nav items.
- Use URL search params for state that should survive reload or a deep link: `?new=1` opens the create form (the dashboard's quick actions link to `/income?new=1`), `?expense=<id>` opens a detail drawer. Derive the state from the params; don't copy it into state with effects.

## 4. Page

```tsx
export function ThingsPage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const list = useThings(business.id, params);
  // … hooks before any early return
  return <Page title="Things" description="…" actions={<Button …>Add thing</Button>}>…</Page>;
}
```

- **States** (`components/States.tsx`): `ListSkeleton` while pending, `ErrorState` with `onRetry`, `EmptyBlock` with the next action. A filtered empty list says so and offers "Clear search".
- **Layout:** `Page`, `SectionCard`; periods with `PeriodPicker` and `lib/periods.ts`.
- **Phone** (`useIsMobile()`, below 48em): cards instead of tables (`IncomeCards` vs `IncomeTable`), full-width actions, nothing clipped at 390 px. Desktop tables sit in `Table.ScrollContainer`.
- **Money:**
  - Display with `MoneyText` and `formatMoney` (`lib/format.ts`).
  - Inputs use `MoneyInput` (minor units, string parsing).
  - Totals, GST and balances shown as fact come from the API. The invoice editor previews totals through `/invoices/calculate` (`useCalculation`). A client-side preview (like `gstPreview`) must be labelled as one and match the API's rounding, with a test.
- **Charts:** load the `dataviz` skill first. Use `@mantine/charts` with `var(--app-income)` / `var(--app-expense)`, a legend, and a table view toggle (`MonthlyChart` in `features/dashboard/DashboardPanels.tsx`).
- **Copy:** NZ/UK spelling, typographic apostrophes (`can’t`), the product name only through `BRAND` (`branding/brand.ts`).

## 5. Forms and mutations

- Mantine `useForm<Values>({ initialValues, validate: zodValidate<Values>(schema) })` (`lib/forms.ts`). Form field names equal the API's request field names, so server field errors land on the right input.
- Put the form in an inner component of the modal, so it resets on close and initial values are computed after the data loads (`IncomeForm` inside `IncomeFormModal`).
- Mutations use `useApiMutation(fn, { form, successMessage, invalidate, onSuccess })` (`services/api/query.ts`). It shows the toast, puts 422 field errors on `form` and invalidates the listed keys.
- **Invalidation:** after any financial change invalidate the module's keys plus `financialKeys(businessId)` (`features/invoices/api.ts`: invoices, quotes, dashboard, activity, customers). At minimum: `businessKeys.dashboard` and `businessKeys.activityAll`.
- Destructive actions: `confirmAction({ danger: true, … })` (`components/confirmAction.tsx`). Voiding a financial record asks for a reason with `promptReason` (`components/promptReason.tsx`).
- Disable or set `loading` on submit buttons while `isPending`.

## 6. Tests

- **Component tests** (`features/<module>/<module>.test.tsx`):
  - Render with `renderWithProviders(<ThingFormModal opened onClose={…} />)` from `test/render.tsx`.
  - Mock only `useCurrentBusiness`, with `vi.mock('@/features/businesses/api', …)`, and stub `fetch` (`vi.spyOn(globalThis, 'fetch')`) to simulate server responses (see `features/income/income.test.tsx`).
  - Assert real behaviour: validation messages, no request sent while invalid, the request body sent, field errors from a 422.
  - Unit-test the `toInput`/`fromRow` conversions.
- **E2E:**
  - Add the main journey to `e2e/records.spec.ts` or `e2e/bookkeeping.spec.ts`, or a new spec. Phone flows go in `*.mobile.spec.ts`.
  - Start with `startBusinessSetup(page)` and `createBusiness(page, 'Thing Studio')` from `e2e/helpers.ts`.
  - Seed prerequisites through the API with `page.request` and `x-client-kind: web` (`apiCustomer` in `bookkeeping.spec.ts`). See `e2e/CLAUDE.md`.

## 7. Done checks

```bash
npm test -w @solo/client -- src/features/<module>    # quick loop
npm run check
```

Then run the `ui-review` skill for every new route in light, dark and phone layouts, and the affected E2E specs (`/e2e`).
Update the in-app guide in the same commit with the `update-guide` skill: a step for the new screen or tab, in the
screen's own words (`guide.test.tsx` fails without one, and the guide-check hook stops the commit).
Finally, the `update-readme` skill: add the feature to the README's Features list and, for a new module or a
screen that changed noticeably, its screenshot (`npm run readme:screenshots`).
