---
description: Cut a Solo release — review and merge release-please's "chore: release X.Y.Z" pull request, then watch the Release workflow publish it
argument-hint: '[X.Y.Z, optional: force this version]'
---

Release Solo. See `docs/deployment/release-process.md`. Versions are semantic and come from the Conventional Commits since the last release: release-please (`.github/workflows/release-please.yml`) keeps a pull request "chore: release X.Y.Z" up to date on every push to `main`. Nothing is versioned, tagged or built on this machine.

**Only run this when the user has explicitly asked for a release in this conversation.** Merging the release pull request publishes a GitHub release that installed apps are offered through Settings › Updates.

1. **Preconditions:**
   - You are on `main`, up to date with `origin/main` (`git fetch origin`), with nothing unpushed.
   - CI on the latest `main` commit is green: `gh run list --workflow ci.yml --branch main --limit 1`.
   - `/release-check` has passed in this conversation, or the user says it has.
2. **The release pull request:** `gh pr list --label "autorelease: pending" --json number,title,url`. None means no `feat`/`fix`/`perf`/`deps`/breaking commit since the last release: say so and stop (don't invent one).
   - If `$ARGUMENTS` gives a version that differs from the pull request's, add `"release-as": "$ARGUMENTS"` to the package in `release-please-config.json` (a `chore:` commit), push, wait for the pull request to update, and remove `release-as` in a later commit after the release.
   - Read its diff: only versions (`package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock`), `.release-please-manifest.json` and the new `CHANGELOG.md` section. The changelog lines are user-facing: fix wording on the pull request's branch if a line won't read well to a sole trader.
3. **Merge** (only now, with the user's go-ahead in this conversation): `gh pr merge <n> --squash`. The merge commit's subject must stay `chore: release X.Y.Z`.
4. **Watch:** the merge runs `release-please.yml` again. Its `release-please` job tags `vX.Y.Z` and drafts the release; its `build` job calls `release.yml` (version check, Linux/Windows/macOS installers with signed update packages and `latest.json`, then publish).
   - `gh run list --workflow release-please.yml --limit 1`, then `gh run watch <id> --exit-status` in the background. The desktop jobs take the longest.
5. **Report:**
   - On success: `gh release view vX.Y.Z --json url,isDraft,assets --jq '{url, isDraft, assets: [.assets[].name]}'`. Give the release URL and the installer list, and check `latest.json` is among the assets.
   - On failure: `gh run view <id> --log-failed`. Report the failing job and its cause. The release stays a draft. Don't delete the tag, re-run jobs or push fixes without asking.
