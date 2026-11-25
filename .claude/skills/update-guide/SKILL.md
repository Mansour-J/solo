---
name: update-guide
description: Keep Solo's in-app beginner's guide (apps/client/src/features/guide/content.ts) in step with the app — add or change steps for new screens, tabs, buttons, fields and rules, in the app's own words, with links that work and labels the guard test checks. Use in the same change as anything a user can see: a new page, tab, button, field, menu item, notice, setting or tax rule, or a renamed or removed one; and whenever the guide-check hook stops a commit.
---

# Update the guide

The guide (**Guide** in the sidebar, `/guide`) walks a new sole trader through every feature, in the order
they use them, ending with the yearly report. It is part of the product: a feature isn't done until the
guide explains it. Update it **in the same commit** as the change.

## What enforces it

- **`guide.test.tsx`** (part of `npm run check`) fails when:
  - a step links to a route that doesn't exist;
  - a screen in `NAV_ITEMS`, `FOOTER_NAV_ITEMS` or the settings routes has no step linking to it;
  - a **Year end** tab (`YEAR_END_TABS` in `features/yearend/forms.ts`) has no step linking to `/year-end?tab=…`;
  - a `**label**` in the guide doesn't appear anywhere in the client's source (a renamed or removed button,
    field or tab). `Settings › Business details` is checked part by part.
  - the copy uses a straight apostrophe.
- **The guide-check hook** (`scripts/guide-check.mjs`, PreToolUse in `.claude/settings.json`) stops a
  `git commit` whose staged changes touch user-visible client files (`*.tsx`, navigation, routes) without
  `content.ts`. If nothing users see changed (a refactor, tests, the same labels), put a line
  `Guide: unchanged` in the commit message. `node scripts/guide-check.mjs status` runs the same check.

The tests catch missing links and stale labels; they can't tell whether a *new* feature is explained.
That is this skill's job.

## How to update it

1. **Find the chapter.** Chapters follow a sole trader's year (`start`, `business`, `opening`, `customers`,
   `quotes`, `invoices`, `expenses`, `income`, `mileage`, `money`, `planning`, `year-end`, `account`). Add a
   step to the chapter where the user meets the feature; add a chapter only for a new area of work (then
   update the chapter count in `guide.test.tsx`).
2. **Write the step** (`title`, `body`, optional `link` and `tip`):
   - Say what to do on the screen, in order: "Go to **Year end › Assets**, … choose **Save**."
   - Put the app's own words in `**bold**`, exactly as the screen shows them (copy from the component). Read
     the component; don't guess a label.
   - `link.to` is a real route, with a deep link where one exists (`?new=1`, `?tab=…`, `?report=…`).
   - Explain the rule behind it in one sentence where it matters (IRD thresholds, what's claimed, what isn't),
     and say plainly what the app doesn't do. A `tip` holds a useful aside, not the main instruction.
   - NZ/UK spelling, typographic apostrophes (’), no jargon without a glossary entry (`GLOSSARY`).
3. **Changed or removed UI:** search the guide for the old label (`grep -n "Old label" content.ts`) and the
   screen's route, and fix every mention. A removed feature loses its step.
4. **Check:** `npm test -w @solo/client -- src/features/guide`, then look at `/guide` if the layout matters.
5. **Docs that mirror the guide:** the README features list (update-readme skill) and
   `docs/product/ird-rules.md` for tax rules.

## Checklist for a new feature

- [ ] A step (or a sentence in an existing one) says where it is and how to use it, in the app's words.
- [ ] New page or settings section: a step links to it (the test checks this).
- [ ] New Year end tab: added to `YEAR_END_TABS` and linked from a step.
- [ ] Renamed labels updated everywhere in the guide.
- [ ] The dashboard, GST, income tax or year-end steps mention it if it changes what users see there.
