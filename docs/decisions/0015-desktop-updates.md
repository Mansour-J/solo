# 0015 — In-app desktop updates

- **Status:** Accepted
- **Date:** 2026-11-22

## Context

People install the desktop app once and rarely visit a download page again, so fixes reach them late. They
should be able to find out from the app whether a newer version exists, and install it and restart in one
step, without losing their books. The app runs its own PostgreSQL (ADR 0014), so it must stop that cleanly
before an installer replaces the program. The repository is private for now, so release files can't be
downloaded anonymously, and a token can't be built into the app. Hermes Hub solved the same problem
(its ADR 0003); this follows it where it fits.

## Decision

- **Tauri's updater, driven from Rust.** `src-tauri/src/updates/` has three commands: `update_support`
  (whether this build has a feed, and its version), `update_check` (asks the feed, keeps the update it
  found) and `update_install` (downloads, verifies, installs, then stops the local server and restarts).
  The page never calls the updater plugin directly, so the capability grants nothing for it. On Windows
  the installer closes the app itself; `on_before_exit` stops the local server first.
- **Settings › Updates** (desktop only): the version, *Check for updates*, and for a newer version its
  notes and *Update and restart*, with download progress. Nothing is checked or installed in the
  background: the user asked for a button they press.
- **Signed packages.** Updates are verified against the public key in `tauri.conf.json`
  (`plugins.updater.pubkey`) before they're installed. The key pair was made with
  `tauri signer generate`; the private key and its password are in `~/.tauri/solo-updater.key*` on the
  maintainer's machine and in the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Lose them and installed apps can never accept another update:
  keep a copy in a password manager.
- **Release builds only.** `.github/workflows/release.yml` adds the overlay
  `src-tauri/tauri.updater.conf.json` (signed update packages) when the key secret exists, and
  `tauri-action` uploads `latest.json` to the release. Local and CI builds stay unsigned for updates.
  Linux packages update in place: a `.deb` install takes the `.deb` (`dpkg` through a `pkexec` password
  prompt), an AppImage the AppImage. Failures are reported as what to do (connection, security check,
  password), since release builds keep no log.
- **The feed address is a build setting.** The repository variable `UPDATER_ENDPOINT` (the public URL of
  `latest.json`) is written into `plugins.updater.endpoints` at build time. A build without it doesn't
  offer updates: the settings entry is hidden, so there's no button that can't work. Each build keeps the
  address it was made with.

## Consequences

- Until the feed is public, release builds don't show Settings › Updates. To switch updates on, publish
  the installers and `latest.json` somewhere public (a public releases repository, or a storage bucket;
  `latest.json`'s URLs must point there too), set `UPDATER_ENDPOINT`, and release. Builds made before
  then never update themselves.
- On Linux, installing a `.deb` update asks for the computer's password.
- Rollback: unpublish the faulty release so `latest.json` points at the previous one, then release a
  higher patch version. The updater only moves forward.
- `requireSignedVersion` is on: the version inside the package's signature must match the version the
  feed announces, so a feed can't pass an older signed package off as an update. The Tauri CLI records
  the version when it signs, and no release was signed before this, so nothing is left behind.

## Alternatives considered

- **The updater's JavaScript API** (as in Hermes Hub): the page would need the updater and process
  permissions, and it couldn't stop the local database before restarting.
- **Background checks and a prompt:** useful later; the request was a button in Settings.
- **A token in the app to read private releases:** a secret in the client.
- **Download page only:** users rarely check, and old versions linger.

## Update (2026-11-26): a public repository

The repository is now public (the private one, with the full history, is `solo-old`), so its releases
can serve the feed directly: `UPDATER_ENDPOINT` =
`https://github.com/Mansour-J/solo/releases/latest/download/latest.json`. The release workflow runs
on tags again (hosted runners are free on public repositories), and every release is built there. The first updater key was lost (repository secrets can't be read back), before any
installed app had a feed, so a new pair was generated on 2026-11-26: its public key is in
`tauri.conf.json`, the private key and password are in the `TAURI_SIGNING_*` secrets and in
`~/.tauri/solo-updater.key*` on the maintainer's machine. v0.4.0 is the first release with the feed.
