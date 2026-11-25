---
description: Cut a Solo release — set the version, commit, tag vX.Y.Z, push the tag and watch the Release workflow
argument-hint: '<X.Y.Z>'
---

Release version `$ARGUMENTS`. See `docs/deployment/release-process.md`.

**Only run this when the user has explicitly asked for a release in this conversation.** Pushing the tag publishes versioned images to GHCR and a GitHub release, and runs the Linux, macOS and Windows runners (free: the repository is public).

1. **Preconditions:**
   - `$ARGUMENTS` is `X.Y.Z`, and the tag `vX.Y.Z` doesn't exist yet (`git tag -l vX.Y.Z`, `git ls-remote --tags origin vX.Y.Z`).
   - You are on `main`, up to date with `origin/main`, with a clean working tree.
   - `/release-check X.Y.Z` has passed in this conversation, or the user says it has.
   - Pushing needs the personal GitHub identity: run `mansour-ssh` first (your work SSH alias switches back afterwards).
2. **Version:** `npm run release:version -- X.Y.Z`. It sets the three `package.json` files, the lockfile and the three `Cargo.toml`/`Cargo.lock` pairs (API, local server, desktop shell). Check `git diff --stat` shows only those files. On Windows, check `package-lock.json` changed only version lines: some npm versions drop the `libc` markers of platform packages there, which would break Linux installs (restore it and bump the versions by hand).
3. **Commit and tag:** `git commit -am "Release vX.Y.Z"`, then `git tag -a vX.Y.Z -m "Release vX.Y.Z"`. The tag must be annotated: `git push --follow-tags` skips lightweight tags.
4. **Push:** `git push origin main`, then `git push origin vX.Y.Z`. If `main` is protected, stop before tagging: push the version bump on a branch, open a pull request, and tag the merged commit on `main` once the user has merged it.
5. **Watch** `.github/workflows/release.yml`:
   - `gh run list --workflow release.yml --limit 1` to get the run ID.
   - `gh run watch <id> --exit-status` in the background. The desktop jobs take the longest.
   - The workflow checks the versions, drafts the release with notes from the commit subjects, pushes the `solo-api` and `solo-web` images, builds the installers and publishes the release when everything succeeded.
6. **Report:**
   - On success: `gh release view vX.Y.Z --json url,isDraft,assets --jq '{url, isDraft, assets: [.assets[].name]}'`. Give the release URL and the installer list.
   - On failure: `gh run view <id> --log-failed`. Report the failing job and its cause. The release stays a draft. Don't delete the tag, re-run jobs or push fixes without asking.
