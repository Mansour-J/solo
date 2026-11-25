# Release process

One version everywhere: the npm packages, the API crate and the desktop crate.

```bash
npm run release:version -- 0.2.0          # sets the version and updates lockfiles
git commit -am "Release v0.2.0"
git tag -a v0.2.0 -m "Release v0.2.0"   # annotated: lightweight tags aren't pushed with --follow-tags
git push origin HEAD v0.2.0
```

The tag runs `.github/workflows/release.yml`:

1. Checks the tag matches every version, drafts a GitHub release with notes from the commit subjects
   since the previous tag and upgrade instructions.
2. Builds desktop installers: Linux (deb, rpm, AppImage), Windows (MSI, NSIS), macOS (universal DMG).
3. Publishes the release when everything succeeded.

## Before tagging

- CI is green on `main`, including e2e and migration upgrade checks.
- `docs/product/roadmap.md` and the README status table are current.
- Release notes will mention any manual step (a long migration, say).

## Code signing

Installers are unsigned until certificates are configured (missing secrets simply skip signing); users then see SmartScreen or Gatekeeper
warnings. To sign, add repository secrets:

- **macOS** (signing and notarisation): `APPLE_CERTIFICATE` (base64 .p12),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`
  (app-specific password), `APPLE_TEAM_ID`.
- **Windows**: `WINDOWS_CERTIFICATE` (base64 .pfx), `WINDOWS_CERTIFICATE_PASSWORD`.

No secret is ever written into a build artifact.

## In-app updates

Desktop apps update themselves from Settings › Updates ([ADR 0015](../decisions/0015-desktop-updates.md)):

- **Signing (set up):** repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
  hold the updater key; the public key is in `apps/client/src-tauri/tauri.conf.json`. With them, a release
  signs the update packages and uploads `latest.json`. Keep the private key and password in a password
  manager: if they're lost, installed apps can never update again. (Since the move to the public
  repository on 2026-11-26 the secrets aren't set: the old repository's can't be read back. Recover the
  key, or, while no installed app has a feed, generate a new pair and put its public key in `tauri.conf.json`.)
- **Feed (not yet):** the repository is public, so its releases can be the feed: set the repository
  variable `UPDATER_ENDPOINT` to `https://github.com/Mansour-J/solo/releases/latest/download/latest.json`
  once releases are signed. Only builds made after that offer updates; builds without it don't show
  Settings › Updates.
- **Rollback:** unpublish the faulty release, then release a higher patch version.

## Costs

The repository is public, so GitHub-hosted runners are free, macOS and Windows included. Every build and
release runs on GitHub; nothing is built for release on a maintainer's machine.
