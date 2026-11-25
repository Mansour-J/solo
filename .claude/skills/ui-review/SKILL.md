---
name: ui-review
description: Visually verify Solo screens before calling UI work done — screenshots in light, dark and phone layouts with console-error capture, and a checklist for clipping, dark tokens, phone layout, loading/empty/error states, dead UI, fake data, NZ/UK copy, accessibility and charts. Use after building or changing any page, modal, drawer, chart or layout.
---

# UI review

Type checks and tests don't catch layout problems. Look at the screens.

## 1. Screenshot

The web app and the API must be running (`npm run dev` on port 5173, and `POSTGRES_PORT=55432 CORS_ALLOWED_ORIGINS= node scripts/api-dev.mjs`). Start them in the background if they aren't.

```bash
npm run snap -- /dashboard /invoices /expenses               # desktop light, 1440×900
npm run snap -- --dark /dashboard
npm run snap -- --mobile /dashboard                          # Pixel 7, touch
npm run snap -- /invoices/{invoice} 'add-expense:/expenses?new=1'   # seeded IDs, deep-linked form
npm run snap -- --empty /income /customers                   # a business with no records
```

- `scripts/snap.mjs` creates a throwaway local business whose records are all labelled "(test)". Never present those figures as real, and never point it at a non-local server (it refuses).
- Images go to `/tmp/solo-snaps/<name>[_m][_d].png`. The script waits until no loader or `aria-busy` skeleton remains and prints console and page errors: any error is a finding.
- Open every image with the Read tool and actually look at it.
- For states behind interaction (an open menu, a confirmation, a 422 on a form), prefer a `?new=1` or `?expense=<id>` deep link. Otherwise write a short Playwright snippet using `e2e/helpers.ts`, and delete it afterwards.

## 2. Checklist

- **Layout:**
  - No clipped or overlapping text, badges, amounts or buttons. Flex rows need `minWidth: 0` on the shrinking child and `flexShrink: 0` on amounts and badges.
  - Long names truncate (`clamp-1`, `lineClamp`). The seeded "Tōtara Design and Architecture Studio Limited (test)" is there to test this.
  - Amounts are right-aligned in tables and never wrap.
  - Page actions wrap under the title on narrow widths (`Page`).
- **Dark mode:**
  - Surfaces use the tokens in `apps/client/src/theme/global.css` (`--app-surface-*`, `--app-border*`); no hard-coded light colours.
  - No `light-dark()` in `.css` files.
- **Phone** (`--mobile`):
  - Tables become cards, and nothing scrolls sideways except an intentional `Table.ScrollContainer`.
  - The navigation drawer opens from "Toggle navigation".
  - Modals and drawers fit the screen; primary actions are reachable.
- **States:** a loading skeleton with stable height; an empty state with the next action (check with `--empty`); an error state with "Try again"; read-only access hides write actions.
- **No dead UI:**
  - Every visible button and link works end to end.
  - Nav entries in `apps/client/src/app/nav.ts` only for working modules.
  - No "coming soon", disabled placeholders or hidden stubs.
- **No fake data:** every figure comes from the API, and from the journal for totals. Nothing is invented, sampled or hard-coded. A preview or estimate is labelled as one, and a success toast only appears for something that happened.
- **Copy:**
  - NZ/UK spelling (organise, colour, cancelled).
  - Typographic apostrophes (`don’t`) and en dashes in ranges.
  - Dates and money through `lib/format.ts`.
  - The product name only from `BRAND`.
- **Accessibility:**
  - Icon-only buttons have an `aria-label`; focus rings are visible.
  - Inputs have labels, and errors appear next to the field.
  - Colour is never the only signal (status badges have text).
- **Charts** (load the `dataviz` skill):
  - Income and expense series use `var(--app-income)` / `var(--app-expense)` only. These are validated for both schemes and colour-vision deficiency; run the dataviz validator before adding any colour.
  - A legend or direct labels, a tooltip, and a table view of the same data (`MonthlyChart`).
  - Check the chart in both light and dark mode.

## 3. Report

List the findings with the screenshot each came from, fix them, and re-shoot to confirm. If the screen is
one the README shows (or should), follow the `update-readme` skill.
