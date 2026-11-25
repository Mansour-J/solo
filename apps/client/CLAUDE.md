# Web client (React 19 + Mantine 9)

The app runs in the Tauri webview (`src-tauri/`, see its `CLAUDE.md`); in a browser only for development and end-to-end tests (Vite proxying an API). There is no sign-in (ADR 0016). All paths below are under `src/`.

## Layout

- `app/`:
  - `App.tsx` waits for the desktop app's local server (`features/server`), then loads the owner's profile before any route renders.
  - `router.tsx` is the hash router (`createHashRouter`); `RequireBusiness` in `guards.tsx` sends a first start to onboarding.
  - `nav.ts` holds `NAV_ITEMS` (the modules) and `FOOTER_NAV_ITEMS` (Guide and Settings, pinned to the bottom), read by the sidebar and the phone drawer in `app/layout/AppLayout.tsx`.
- `features/guide/`: the beginner's guide. Its text is data in `content.ts`, using the screens' own labels; when you rename a button, field or tab, or add a feature, update the guide in the same commit (`update-guide` skill). `guide.test.tsx` checks every link goes to a real route, every screen, settings page and Year end tab (`YEAR_END_TABS`) has a step, and every `**label**` still exists in the app; a commit hook (`scripts/guide-check.mjs`) stops UI changes committed without it.
- `features/<module>/`: `api.ts` (query keys and hooks), form modules (`*Form.ts`), pages, modals, `routes.tsx` and tests. Features may use another feature's exported hooks and pickers (`CustomerSelect`, `financialKeys`), not its internals.
- `components/`: `Page`, `States.tsx` (`EmptyBlock`, `ErrorState`, `ListSkeleton`, `SectionCard`), `Money.tsx` (`MoneyText`, `MoneyInput`, `StatusBadge`), `PeriodPicker`, `confirmAction`, `promptReason`, `Logo.tsx` (`LogoMark`, `Wordmark`).
- `services/api/`:
  - `client.ts`: the typed `api` client, `unwrap`, `fetchBlob`, `uploadFile`, `connectBackend` (the local server's address and key)
  - `query.ts`: `useApiMutation`, the query client
  - `errors.ts`: `ApiError`, `errorMessage`.
- `services/platform.ts`: `isTauri`, `CLIENT_KIND`, `backendStatus`, `restartApp`, the update commands, `openExternal`, `saveBlob`. Platform differences go here, not into features.
- `stores/session.ts`: Zustand, holding whether the profile has loaded and the selected business only. Server data stays in TanStack Query.
- `lib/`: `format.ts` (dates and money in the user's preferred formats), `forms.ts` (`zodValidate`), `periods.ts`.
- `theme/`: `theme.ts`, `palette.ts` (the Mantine `brand` colour generated from the brand colour), and `global.css`, which holds the light and dark tokens (`--app-*`).

## Rules

- **Every request** goes through `api` from `services/api/client.ts`, which sends `x-client-kind` (and the local server's `x-api-key`). Don't call `fetch` directly; binary downloads and uploads use `fetchBlob` and `uploadFile`.
- **Business context:** inside `RequireBusiness`, `useCurrentBusiness()` gives `id`, `currency`, `today`, `gst_registered` and `current_financial_year`. Default dates come from `business.today`, never the browser clock. There are no roles: every action is available.
- **CSS:** never use `light-dark()` in `.css` files: postcss-preset-mantine rewrites it into selectors that don't match. Define tokens per scheme in `theme/global.css`.
- **URL state:** derive modals and drawers from search params (`?new=1`, `?expense=<id>`). Put modal content in an inner component so it resets on close.
- **Public config:** only `VITE_*` values reach the bundle, so they must never be secrets. Vite injects the brand as `__BRAND__` (read it through `BRAND`).

## Tests

- Vitest + Testing Library in jsdom.
  - Render with `renderWithProviders(ui, { path })` from `test/render.tsx`.
  - Mock `useCurrentBusiness`, and stub `fetch` only to simulate server responses.
  - Assert what the user sees and what is sent.
- `npm test -w @solo/client -- src/features/<module>` runs one feature; `npm run check` runs everything (typecheck, lint with zero warnings, Prettier, Vitest).
- Journeys: `e2e/` (see `e2e/CLAUDE.md`).

Skills: `add-feature-page` for screens, `ui-review` (with `/snap`) before calling UI work done.
