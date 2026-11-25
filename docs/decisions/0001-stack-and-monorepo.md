# 0001 — Stack and repository structure

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

The product brief fixes the stack (Rust, Axum, SQLx, PostgreSQL, Flyway, React, TypeScript, Mantine,
Vite, Tauri 2, Docker, GitHub Actions) and asks to reuse Hermes Hub's conventions where they fit.
Hermes Hub is a client-first app whose backend is an in-app TypeScript demo server; it has a
PostgreSQL + Flyway schema but no Rust API.

## Decision

- **Modular monolith.** One Axum service layered `api → services → repositories → domain`, with
  `auth`, `middleware` and `jobs` alongside. Modules are files within each layer, so a feature is a
  vertical slice through the layers without separate deployables.
- **Monorepo:** `apps/client` (React app + `src-tauri`), `services/api`, `packages/api-client`,
  `database`, `infra`, `branding`, `docs`, `e2e`. npm workspaces for the TypeScript packages; the two
  Rust crates are independent Cargo projects, so API builds don't need desktop (WebKit) libraries.
- **Client:** Hermes Hub's stack and conventions unchanged (React 19, TS 6 strict, Mantine 9,
  TanStack Query, Zustand, zod, React Router hash routing, ESLint with React Compiler rules).
- **No demo backend.** The client always talks to the real API; financial screens never show
  invented figures.
- **One web origin.** nginx serves the built app and proxies `/api` to the API, so browser sessions
  use first-party HttpOnly cookies and need no CORS.

## Consequences

- Local development needs Docker (PostgreSQL, Mailpit) and a Rust toolchain; `docker compose up`
  alone runs everything for evaluation.
- Splitting a module into its own service later is possible because modules only talk through
  services, but nothing is prematurely distributed.

## Alternatives considered

- **Cargo workspace including the Tauri crate:** shared lockfile, but every API build and CI job
  would need WebKit system libraries.
- **Keeping Hermes Hub's single-package layout:** doesn't fit two deployables (API, web) plus a desktop app.
- **Microservices per module:** operational cost with no benefit at this scale.
