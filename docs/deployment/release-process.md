# Release process

One version everywhere: the npm packages, the API crate, the local server and the desktop crate. Versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html), commits follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) (CONTRIBUTING.md), and
[release-please](https://github.com/googleapis/release-please) turns one into the other. Nothing is
versioned, tagged or built by hand.

1. **Every push to `main`** runs `.github/workflows/release-please.yml`. It reads the commits since the
   last release and keeps a pull request "chore: release X.Y.Z" up to date: the next version (`feat` →
   minor, `fix`/`perf`/`deps` → patch, breaking → major, or minor while the version is 0.x) in every
   `package.json`, `package-lock.json`, `Cargo.toml` and `Cargo.lock` (`release-please-config.json`), and
   the new section at the top of `CHANGELOG.md`. Edit the pull request (the changelog wording, say) before
   merging if needed.
2. **Merging it** tags `vX.Y.Z`, drafts the GitHub release with that changelog section as its notes, and
   calls `.github/workflows/release.yml`, which checks the versions, builds the desktop installers (Linux
   deb, rpm and AppImage; Windows MSI and NSIS; macOS universal DMG) with signed update packages and
   `latest.json`, uploads them, and publishes the release.
3. **Apps update:** Settings › Updates in installed apps finds the new version through `latest.json`.

To release a specific version (a first 1.0.0, say), add `"release-as": "X.Y.Z"` to the package in
`release-please-config.json` and remove it after that release. Pushing a `vX.Y.Z` tag by hand still runs
the Release workflow (its versions must already match). Running it from the Actions tab on a branch builds
the installers as artifacts without releasing.

## Before merging a release pull request

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
  signs the update packages and uploads `latest.json`. The key pair was made on 2026-11-26 (the first
  one was lost before any app had a feed); the private key and its password are in
  `~/.tauri/solo-updater.key*` on the maintainer's machine. Keep a copy in a password manager: if
  they're lost, installed apps can never update again.
- **Feed (live from v0.4.0):** the repository variable `UPDATER_ENDPOINT` is
  `https://github.com/Mansour-J/solo/releases/latest/download/latest.json`, so every build from the
  Release workflow checks the latest published release. Builds made before v0.4.0 have no feed and
  don't show Settings › Updates: their users install v0.4.0 by hand once.
- **Rollback:** unpublish the faulty release, then release a higher patch version.

## Costs

The repository is public, so GitHub-hosted runners are free, macOS and Windows included. Every build and
release runs on GitHub; nothing is built for release on a maintainer's machine.
