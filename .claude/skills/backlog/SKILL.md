---
name: backlog
description: Read, add or update Solo backlog items (SL-…) in docs/backlog/backlog.html — the interactive page whose embedded JSON holds every item's status, type, priority, size, dependencies and acceptance criteria. Use whenever work starts, finishes or changes scope on a backlog item, a follow-up or idea needs recording, a criterion is met, or someone asks what is open, ready, blocked or next.
---

# Solo backlog

The backlog is **`docs/backlog/backlog.html`**: one self-contained page (no build, no dependencies) that
renders the JSON in its `<script type="application/json" id="backlog-data">` block. **An item's status
lives only there**; the roadmap (`docs/product/roadmap.md`) gives the phase overview. There is no
Markdown copy any more: don't recreate `backlog.md`.

## Reading it

- **People:** open the file in a browser. List (grouped by phase, status, priority, type or size),
  Board (columns by status) and Guide (the rules and the next free ID per phase). Filter with the
  chips (several values per facet), search with `/`, hide done and dropped, open an item for its
  criteria, code, notes, dependencies and what it's needed by (`j`/`k` step through, `Esc` closes).
  The filters live in the URL hash, so a link such as
  `backlog.html#status=Ready,Idea&priority=P1&open=1` or `backlog.html#item=SL-005` reopens that view.
- **Agents:** don't read the whole HTML. Use the script:

  ```bash
  npm run backlog -- list open                    # everything not Done, Dropped or Parked
  npm run backlog -- list status=Ready priority=P0,P1 section=phase-7
  npm run backlog -- show SL-005                  # one item as JSON
  npm run backlog -- next phase-7                 # next free ID in a section
  npm run backlog -- check                        # validate (also part of npm run check)
  ```

  To edit, find the item with `grep -n '"id": "SL-005"' docs/backlog/backlog.html` and read around it.

## The data

```jsonc
{
  "id": "SL-123",               // SL-<section digit><two digits>; never reused
  "title": "Short imperative title",
  "section": "phase-7",         // a sections[].id: foundation, phase-3 … phase-8
  "status": "Ready",            // Idea | Ready | In progress | In review | Blocked | Parked | Done | Dropped
  "statusNote": "why",          // optional; required for Dropped (why) and for Blocked unless notes say why
  "type": "Feature",            // Feature | Tech | Decision (produces an ADR) | Spike | Chore
  "priority": "P1",             // P0 blocks the phase · P1 needed · P2 should have · P3 nice to have
  "size": "M",                  // S ≤ 1 day · M 2–3 days · L a week · XL split it
  "dependsOn": ["SL-101"],      // [] for none; every ID must exist
  "summary": "The problem and the outcome.",   // optional
  "criteria": [{ "done": false, "text": "Verifiable by a test or a demo" }],  // [] for none
  "code": "`paths/that/change`",  // optional; may end with "Decision: [ADR 0016](../decisions/0016-….md)."
  "notes": "Decisions, links, open questions",  // optional
  "labels": ["Third-party integration"]  // optional; each one of the vocabulary `labels`
}
```

- Only the keys above are allowed; leave optional text keys out rather than empty.
- Text takes inline Markdown: `` `code` ``, `**bold**`, `*italic*`, `[link](relative/to/docs/backlog)`,
  and bare item IDs (`SL-005`) become links. Paragraphs are separated by `\n\n`. NZ/UK spelling.
- Items sit in the `items` array grouped by section, in ID order. Sections (`sections`) carry
  `id`, `phase`, `title`, `idPrefix` (the ID digit) and `intro` paragraphs. The vocabularies
  (`statuses`, `types`, `priorities`, `sizes`, `labels`) and the Guide text (`guide`) are data too: the
  page and the checker read them, so change a vocabulary there, never in the page's script.

## Adding an item

1. `npm run backlog -- next <section>` for the ID. A new phase: add a `sections` entry (next
   `idPrefix`) and a matching phase in `docs/product/roadmap.md`.
2. Insert the object after the last item of its section. New ideas start as `Idea`; `Ready` needs
   criteria and known dependencies.
3. Validate and format (below).

## Updating an item

- **Starting work:** `In progress`. Built and waiting on review or merge: `In review`.
- **Criteria:** set `"done": true` only when a test or demo shows it. Add criteria discovered while
  building rather than widening existing ones.
- **Done** means every criterion ticked and verified. When an item ships with a deliberate
  remainder, either split the remainder into a new item (preferred) or keep it unticked and say so
  in `statusNote` (the checker warns, which is fine).
- **Partly done** isn't a status: use `In progress` with a `statusNote` such as
  `"income tax done; ACC levies to do"`.
- **Parked / Blocked / Dropped:** say why (and for Parked, where the work is) in `statusNote` or
  `notes`. Never delete an item or reuse its ID; drop it instead.
- Add `code` paths and ADR links when they exist; follow-ups become new `Idea` items.
- After a phase changes, check the roadmap still matches (the `/phase` and `/sync-docs` commands do).

## Republishing the shared copy

The page is also published as a private claude.ai artifact, https://claude.ai/artifact/1YuYxV3QqVkS9jVMFBnAy6,
so it can be opened outside the repository. It is a snapshot: republish it whenever a backlog change
reaches `main`. A Claude Code hook (`.claude/settings.json` → `npm run backlog:publish -- hook`) checks
after every `git push` to `main` and, when the pushed page differs from what was last published, writes
the copy and tells you to republish. Then:

1. `npm run backlog:publish -- export` (the hook has already run it): writes
   `dist/backlog/solo-backlog.html` without the document skeleton and with repository links pointing at
   GitHub.
2. Artifact tool: `action: "read"` on the URL (required before publishing from a new conversation),
   then publish `file_path: dist/backlog/solo-backlog.html` with `url` set to the URL. Never create a new
   artifact for it, and never pass `icon`.
3. `npm run backlog:publish -- published` records what is now live (`dist/backlog/published.sha256`,
   local and ignored by git). `npm run backlog:publish -- status` says whether the copy is current.

The page hides its *Copy link* button inside the viewer's frame and tolerates the frame refusing
history changes, so the published copy needs no other edits.

## Validate and format

```bash
npm run backlog -- check                  # errors fail; warnings are informational
npx prettier --write docs/backlog/backlog.html
```

`check` catches bad JSON, unknown keys or values, duplicate or misplaced IDs and missing
dependencies. Never put `</script` inside the data. The page itself (styles and script) only
changes to change how the backlog is shown; open it in a browser afterwards in light, dark and
phone widths.
