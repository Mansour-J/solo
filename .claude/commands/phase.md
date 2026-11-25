---
description: Plan and deliver the next Solo roadmap phase, or a named item from it, end to end, then update the roadmap, README status, features and screenshots
argument-hint: "[phase number or item, e.g. 3 or 'recurring invoices']  (default: the phase marked 'next')"
---

Work on roadmap item **$ARGUMENTS** (default: the phase marked "(next)" in `docs/product/roadmap.md`).

1. **Find it:** read `docs/product/roadmap.md` and the phase's items in the backlog (`npm run backlog -- list section=phase-<n>` and `show SL-…`: status, criteria, dependencies; `backlog` skill). Skip items that are `Parked` or `Blocked` unless the user says otherwise. Read what the docs already say about it: `docs/architecture/financial-model.md` (planned postings), "Planned tables" in `docs/architecture/database.md`, `docs/product/overview.md`.
2. **Check readiness:**
   - Earlier phases should be ✅. If one isn't, stop and say so, unless the user insists.
   - Tax functionality (Phase 8) comes last, after the core product is stable. Don't pull tax rules into earlier phases.
   - Anything that needs a decision outside the repository (IRD rates or rules, legal wording, a paid provider) goes to the user as a question, not a guess. Rates and thresholds are versioned data with effective dates, never constants in code.
   - Check `git status` and the branch for work already in progress on this item, and build on it rather than starting again.
3. **Plan vertical slices.** Each slice works end to end: migration, ledger postings, endpoints, screen, tests. Map every roadmap bullet to a slice, with the files it touches and the tests that prove it. Confirm that the files you plan to extend exist. Show the plan before implementing a whole phase.
4. **Implement** following `CLAUDE.md` and the nested `CLAUDE.md` files, with the skills:
   - `add-migration`: new tables and constraints
   - `ledger-posting`: anything that affects the books
   - `add-api-endpoint`: domain → repositories → services → api, integration tests, `npm run api:client`
   - `add-feature-page`: the client screen, nav entry and tests
   - `ui-review`: screenshots and the visual checklist.
5. **Definition of done** (`CONTRIBUTING.md`), for every slice:
   - UI, API and migration exist; validation runs on both sides with field errors on the right inputs.
   - Rules live in `services/api/src/domain` with unit tests; every figure comes from the API, and reports from the journal.
   - Tenant isolation is enforced (`begin_business`) and covered by a cross-tenant integration test.
   - Loading, empty and error states; light, dark and phone layouts checked with `/snap`.
   - A Playwright journey in `e2e/` for the main flow.
   - No dead UI: the nav entry and every button work end to end. No fake data: nothing invented, sampled or hard-coded is shown as the user's numbers.
6. **Verify:** `/check full`, then the `ui-review` skill for every new or changed screen.
7. **Finish:**
   - `docs/backlog/backlog.html` (`backlog` skill): set each item to `In progress` when you start it; at the end tick each acceptance criterion that is demonstrably met and set the item's status; add follow-ups as `Idea` items with the next free ID (`npm run backlog -- next <section>`); run `npm run backlog -- check`.
   - `docs/product/roadmap.md`: mark the phase ✅ only if every bullet is done; otherwise list what remains. Move "(next)" on and update "Known limitations".
   - Guide: the `update-guide` skill (a step for every new screen, tab and rule, in the app's own words).
   - README: the `update-readme` skill (Status table, Features list, screenshots regenerated with `npm run readme:screenshots`).
   - `docs/architecture/database.md` (tables per migration), `docs/architecture/financial-model.md` (postings), `docs/development/testing.md` (journeys), and an ADR in `docs/decisions/` for any significant decision.
   - Run `/sync-docs`.
   - Summarise what changed, test results, and follow-ups. Add follow-ups to the roadmap under the right phase.

Don't commit unless the user asks.
