---
description: Screenshot Solo routes in light, dark and phone layouts and review them visually
argument-hint: '<route> [route...]  e.g. /dashboard /invoices /expenses  (optional: --empty, --fresh)'
---

Visually review these routes: `$ARGUMENTS` (default: `/dashboard /invoices /expenses`).

1. Make sure the web app and API are up:
   - `curl -s -o /dev/null -w '%{http_code}' http://localhost:5173` and `curl -s -o /dev/null -w '%{http_code}' http://localhost:5173/health/ready` must both print 200.
   - If not, start the local server (`npm run local:serve`, which prints its URL) and then `VITE_DEV_API_PROXY=<its URL> npm run dev`, in the background, and wait for them.
2. For the routes, run:
   - `npm run snap -- <routes>`
   - `npm run snap -- --dark <routes>`
   - `npm run snap -- --mobile <routes>`

   Pass `--empty` or `--fresh` through. The script creates a throwaway test business on the local server (see the header of `scripts/snap.mjs`): every record in it is labelled test data. `--empty` gives a new business with no records, for empty states.

3. Open every PNG in `/tmp/solo-snaps/` that this run wrote, with the Read tool, and look at it. Note any console or page errors the script printed.
4. Review against the checklist in the `ui-review` skill: clipping, dark-mode tokens, phone layout, states, no dead UI, no fake data, copy, accessibility, charts.
5. Report findings per screenshot. Fix them only if asked, or if they are clear regressions from the current task.
