---
name: update-readme
description: Keep Solo's README current — the logo, the collapsible "Features" list, the collapsible "Screenshots" gallery (regenerated with npm run readme:screenshots) and the "Status" table — after any change users can see. Use whenever a phase, module, page, report or user-visible feature is added, changed or removed, and before a release.
---

# Update the README

The README is the first thing people see. It must describe what Solo does today, honestly, and show
the current screens. Update it in the same branch as the change, not later. The in-app guide changes with
it: use the `update-guide` skill for the same change.

## 1. Features (`## Features`, inside `<details>`)

- One bold group per module (Accounts and businesses, Invoices…, Expenses and income, Mileage,
  Allocations, Reports, Everywhere), with short bullets of what a user can do.
- List only what works end to end now (the roadmap marks it ✅). No "coming soon", no planned work:
  that belongs in the Status table and the roadmap. Check a claim against the code before writing it
  (an endpoint, a page, a test), e.g. `grep -rn` for the route.
- Keep the honesty lines: allocations never move money, GST and tax figures are informational.
- Update the summary line's phase range (`(phases 0–5)`).

## 2. Screenshots (`## Screenshots`, inside `<details>`)

```bash
# Needs Vite on 5173 proxying an API (npm run local:serve, then VITE_DEV_API_PROXY=<its URL> npm run dev).
npm run readme:screenshots      # fresh "(test)" business → docs/screenshots/*.png (light, dark, phone)
```

- The shot list lives in `scripts/readme-screenshots.mjs`; keep it and the README table in step
  (same file names, captions that say what the screen shows).
- Add a shot for a new module or a screen that changed noticeably; remove shots of removed screens
  and delete their files.
- Open every image with the Read tool before committing. No "Confirm your email" banner (Mailpit must
  be running), no clipped layouts, no console errors. A screen that looks broken is a bug to fix (or
  a backlog item), not something to show.
- Screenshots use seeded test records labelled "(test)"; the gallery's summary says so. Never stage real
  data.
- Phone shots go in the last row at `width="200"`.

## 3. Status and the rest

- The "Status" table matches `docs/product/roadmap.md` (Done / Next / Planned).
- The logo is `branding/assets/logo.svg`; if branding changes, check it still renders on GitHub.
- Commands, stack and documentation links stay accurate (the `/sync-docs` link check covers links).

## 4. Check

- Preview the Markdown (GitHub renders `<details>` and the table): blank lines around Markdown inside
  `<details>`, image paths relative to the repository root.
- `npx prettier --check README.md` passes.
