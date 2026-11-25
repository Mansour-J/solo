---
description: Regenerate generated files and check Solo's documentation against the code (API client, brand, links, backlog, roadmap, README status, migration table)
---

Bring the generated files and docs back in line with the code.

1. **API client:** run `npm run api:client`, then `git status --short packages/api-client`. Report whether it changed: CI's `contract` job fails on an uncommitted diff.
2. **Brand:** run `npm run brand:sync -- --check` (CI's `web` job runs it). If it fails, run `npm run brand:sync` and report the change to `apps/client/src-tauri/tauri.conf.json`.
3. **Links:**
   - Find every relative Markdown link in `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `**/CLAUDE.md`, `.claude/**/*.md` and `docs/**/*.md`, and check that each target exists.
   - Check that file paths in backticks (e.g. `services/api/src/services/ledger.rs`) exist, and that named functions, components and scripts still do (`grep -rn`).
   - Fix broken references.
4. **Roadmap and status:**
   - `npm run backlog -- check` passes. Each roadmap phase's state agrees with its backlog items (`npm run backlog -- list section=phase-<n>`): a phase marked ✅ has no open P0/P1 items, and items whose work is merged are `Done` or `In review` (`backlog` skill).
   - `docs/product/roadmap.md`: a phase is marked ✅ only when every bullet works end to end (UI, API, migration, tests). "Known limitations" are still true.
   - README "Status" table matches the roadmap (Done / Next / Planned), and its Features list and Screenshots are current (`update-readme` skill).
   - A module listed as done has its nav entry in `apps/client/src/app/nav.ts`; a nav entry exists only for a module that works.
5. **Database:** `docs/architecture/database.md` "Tables (current)" has one row per `database/migrations/V<n>__*.sql` with the tables it creates, and "Planned tables" no longer lists tables that exist.
6. **Financial model:** `docs/architecture/financial-model.md`'s ledger table has a row for every `SOURCE_*` constant in `services/api/src/services/ledger.rs` and matches what the `domain::ledger` builders post.
7. **Testing:** "Current journeys" in `docs/development/testing.md` matches the tests in `e2e/*.spec.ts`.
8. **ADRs:** `docs/decisions/README.md` lists every `docs/decisions/NNNN-*.md` with its status.
9. Report what you changed and anything that needs a human decision.
