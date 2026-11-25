---
description: Build and/or launch the Solo Tauri desktop app (dev, release, build, or headless smoke test)
argument-hint: '[dev|release|build|smoke]  (default: release if a release binary exists, else dev)'
---

Run the desktop app. Mode: `$ARGUMENTS`. See `apps/client/src-tauri/CLAUDE.md`.

Always `export PATH="$HOME/.cargo/bin:$PATH"` first. The binary is `apps/client/src-tauri/target/release/solo-desktop`.

The desktop app starts its own local server (bundled PostgreSQL and the API, data in the OS's local app-data folder under the bundle identifier) and opens straight into the books: there is no sign-in (ADR 0016).

- **dev:** run `npm run desktop:dev` in the background. Its `beforeDevCommand` starts Vite on port 5173 with `strictPort`, so stop any running `npm run dev` first. The first compile takes a few minutes; tell the user the window will appear when it's ready.
- **build:** run `npm run desktop:build -- --bundles deb` in the background (several minutes: release profile with LTO). Report the bundle path and size from `apps/client/src-tauri/target/release/bundle/deb/`.
- **release:** if the binary is older than the latest change in `apps/client/src` or `apps/client/src-tauri/src`, run **build** first. Then launch it on the user's display in the background with `WEBKIT_DISABLE_DMABUF_RENDERER=1 apps/client/src-tauri/target/release/solo-desktop`, check with `pgrep -a solo-desktop` that it is running, and tell the user it is open. If the process exits with code 0, the user most likely closed the window.
- **smoke** (no visible window): CI's check is `xvfb-run -a env WEBKIT_DISABLE_DMABUF_RENDERER=1 timeout 20 <binary>`; exit code 124 means it stayed up. For a screenshot, run `env -u WAYLAND_DISPLAY GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 xvfb-run -a -s "-screen 0 1440x900x24" bash -c '<binary> & P=$!; sleep 15; import -window root /tmp/solo-desktop.png; kill $P'`, then view `/tmp/solo-desktop.png` and confirm the app rendered (onboarding on a first start, otherwise the dashboard).

The books live in `<local app data>/<bundle identifier>/local/` (the PostgreSQL cluster, files and `local.json`). Deleting that folder starts the app afresh.
