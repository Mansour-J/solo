# Tauri shell (Rust)

The shell around the web client: all product logic is in the React app and the API. It runs the local server (`solo-local`: bundled PostgreSQL and the API) so the books stay on this computer (ADR 0014). There is no sign-in (ADR 0016).

- **Crate:** package `solo-desktop`, lib `solo_desktop_lib` (the suffix avoids a Windows name clash). `src/main.rs` starts it; `src/lib.rs` registers plugins and commands.
- **Local server:** `backend.rs` starts it in the background with its data in `<local app data>/local/` and stops it on `RunEvent::Exit` (SIGTERM, SIGINT and SIGHUP exit the app the same way: `exit_on_signal` in `lib.rs`). `backend_status` gives the webview its address and per-start key (the server refuses requests without it); `restart_app` restarts after a failed start. The client waits for it at start-up (`features/server`).
- **Updates** (`src/updates/`, ADR 0015, desktop only): `update_support`, `update_check` and `update_install` (download, verify, install, stop the local server, restart; a `Channel` reports progress). `lib.rs` sends `update_*` calls to `updates::commands()`. `mod.rs` holds everything testable; the steps that leave the app (stop the local server, run the installer, restart) are behind the `System` trait, really done in `system.rs`. `tests/updates.rs` runs the real updater plugin in a mock app against a signed local feed: keep `npm run desktop:coverage:updates` at 99% or more (`docs/development/testing.md`). Packages are verified against `plugins.updater.pubkey`, and `requireSignedVersion` makes the signed version match the feed's. The feed is `plugins.updater.endpoints`, empty in the repository: release builds get it from the `UPDATER_ENDPOINT` variable, and a build without one hides Settings › Updates. `tauri.updater.conf.json` (signed update packages, needs `TAURI_SIGNING_PRIVATE_KEY`) is added only by the release workflow.
- **Plugins:** `opener` (external links), `updater` (driven from Rust, so the capability grants nothing for it), and `log` in debug builds only.
  - `capabilities/default.json` grants the minimum: `core:default`, `opener:allow-open-url`, `log:default`.
  - A new plugin or command gets the narrowest permission that works, with the reason in your summary.
- **CSP:** `tauri.conf.json` → `app.security.csp`. `connect-src` allows `http://127.0.0.1:*` (the local server's random port) and `http://localhost:8000` for development.
- **Generated configuration:**
  - `productName`, `identifier`, the window title, descriptions and copyright are written by `npm run brand:sync` from `branding/brand.json`. Don't edit them by hand; CI runs `npm run brand:sync -- --check`.
  - Icons are regenerated with `npm run brand:sync -- --icons` from `branding/assets/app-icon.svg`.
  - `version` is read from the root `package.json`; release-please bumps it with the rest (`docs/deployment/release-process.md`).
- **Development:** `VITE_API_BASE_URL` is only the address `npm run desktop:dev` uses before the local server is ready.
- **Builds** bundle PostgreSQL (the version in the root `.cargo/config.toml`), downloaded from GitHub at build time; set `GITHUB_TOKEN` to avoid rate limits.
- `#[cfg_attr(mobile, …)]` keeps the crate mobile-ready, but no Android or iOS project has been generated.

## Build and run

`desktop:dev` uses the same books folder as the installed app (same identifier), so close the installed app first.

```bash
export PATH="$HOME/.cargo/bin:$PATH"
npm run desktop:dev                                    # native window on Vite: web changes hot-reload; Rust changes in src-tauri, services/api, services/local or migrations rebuild and restart it
npm run desktop:build -- --bundles deb                 # release build + .deb in target/release/bundle/deb/
apps/client/src-tauri/target/release/solo-desktop      # on Linux, main.rs sets WEBKIT_DISABLE_DMABUF_RENDERER=1 (blank window otherwise) unless it's set
cargo test --manifest-path apps/client/src-tauri/Cargo.toml
npm run desktop:coverage:updates                       # update tests, failing under 99% of lines (needs cargo-llvm-cov)
```

- **Headless check** (as in CI): `xvfb-run -a env WEBKIT_DISABLE_DMABUF_RENDERER=1 timeout 20 <binary>` must exit with code 124, meaning it was still running. Use `env -u WAYLAND_DISPLAY GDK_BACKEND=x11` when taking a screenshot (`/desktop smoke`), otherwise GTK opens on the real Wayland display.
- The release profile uses LTO, `opt-level = "s"`, `panic = "abort"` and strip, so release builds take several minutes.
- Windows (MSI, NSIS) and macOS (universal DMG) installers are built only by `.github/workflows/release.yml`. Signing switches on when the certificate secrets exist (`docs/deployment/release-process.md`).
