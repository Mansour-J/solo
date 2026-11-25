---
description: Pre-release verification for Solo, following docs/deployment/release-process.md — CI, local gate, versions, builds, desktop smoke test, docs, release notes and honesty checks
argument-hint: '[version, e.g. 0.2.0]'
---

Verify Solo is releasable (version `$ARGUMENTS` if given). Follow `docs/deployment/release-process.md`. Report every step's real outcome. Don't skip steps silently, and don't merge the release pull request (that is `/release`).

1. **CI on `main`:** `gh run list --workflow ci.yml --branch main --limit 3`. The latest run must be green, including the `e2e` and `database` (migration upgrade) jobs. If `gh` can't see the repository, run `mansour-ssh` first.
2. **Local gate:** run `/check full`.
3. **Versions:** the Release workflow fails unless the tag matches `package.json` and both `services/api/Cargo.toml` and `apps/client/src-tauri/Cargo.toml` (`tauri.conf.json` reads the root `package.json`). If a version was given, check those plus `apps/client/package.json` and `packages/api-client/package.json`. Report mismatches: release-please sets every version in its release pull request (`release-please-config.json` lists the files), so a mismatch means a file is missing from that list.
4. **Builds:** `npm run build` (note bundle-size warnings), then `/desktop build` and `/desktop smoke`, and view the screenshot.
5. **Docs:** run `/sync-docs`. `docs/product/roadmap.md`, the README status table, Features and screenshots must be current (`update-readme` skill; `npm run readme:screenshots`).
6. **Manual steps for the release notes.** Find the previous tag (`git describe --tags --abbrev=0`; none yet means the whole history), then:
   - new or renamed environment variables: `git diff <prev>..HEAD -- environment.example docs/deployment/environment-variables.md`
   - new migrations, and whether any rewrites a large table: `git diff --name-only <prev>..HEAD -- database/migrations`
   - the changelog: the `CHANGELOG.md` section in release-please's pull request (`gh pr list --label "autorelease: pending"`), built from the Conventional Commits since `<prev>`. Flag lines that won't read well to users, and user-visible changes committed as `chore`/`refactor` (missing from the changelog).
7. **Updates:** `gh secret list` shows `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, and `gh variable list` shows `UPDATER_ENDPOINT` = `https://github.com/Mansour-J/solo/releases/latest/download/latest.json` (compiled into the installers).
8. **Honesty audit:**
   - No `VITE_*` value holds a secret: `grep -rn "VITE_" apps/client/src environment.example`.
   - `apps/client/src-tauri/capabilities/default.json` is unchanged or minimally widened, with a justification; the CSP `connect-src` in `apps/client/src-tauri/tauri.conf.json` is acceptable for this release.
   - No dead UI or invented figures: every entry in `apps/client/src/app/nav.ts` works end to end, and `grep -rniE "lorem|coming soon|sample data" apps/client/src` is empty.
9. **Release notes draft:** the roadmap items completed since the previous tag, the manual steps from step 6, the code-signing state (installers are unsigned until the certificate secrets exist) and known limitations from the roadmap.
10. **Verdict:** ready / not ready, with blocking issues listed. A release runs the Linux, macOS and Windows runners (free on the public repository).
