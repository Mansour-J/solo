# 0016 — One person, no sign-in: a desktop-only app

- **Status:** Accepted
- **Date:** 2026-11-23

## Context

Solo is for one sole trader keeping their own books. After ADR 0014 the desktop app ran its own
server on the user's computer, yet it still asked them to register, confirm an email it couldn't
send, remember a password it couldn't reset, and sign in and out. Accounts, sessions, roles and the
cloud-server option served a multi-user hosted product that this app isn't.

## Decision

- **No accounts and no sign-in.** The app has one owner: the first row in `users`, created on first
  start by `services::users::ensure_owner` (an installation that had accounts keeps its first user and
  their books). `users` keeps only a name and preferences; migration V14 drops sessions, single-use
  tokens, passwords and email addresses.
- **Every request acts as the owner.** `api::app(state, owner)` attaches `context::OwnerId` to each
  request; the `Owner` and `BusinessContext` extractors read it. There is no authentication or role
  check. Row-level security stays: business data is still read inside `begin_business`, so one
  business's queries can't touch another's.
- **The local server answers only its own app.** It listens on 127.0.0.1 and, with no sign-in,
  requires a random key generated at each start (`x-api-key`, `HttpConfig::api_key`), which only the
  desktop window is given (`backend_status`). Other programs and web pages on the computer can't use
  it. State-changing requests still need `x-client-kind`, so browsers can't send them cross-site
  without a CORS preflight.
- **Desktop only.** The cloud-server option, the Docker images and the server deployment are gone. The
  web build exists for development and end-to-end tests (Vite proxying `solo-local`).

## Consequences

- No registration, sign-in, password reset, email verification, device list or account deletion, and
  no read-only roles. Settings › Profile holds just a name.
- Each test makes its own owner (integration tests name it per request; end-to-end tests work in a
  business of their own), since there's only one owner per database.
- Anyone who can use the computer's account can open the books, as with any other local file. Disk
  encryption and the OS account protect them.
- Sharing books with an accountant, or using them on several devices, would need a new design
  (for example an export, or a synced copy), not the old accounts.

## Alternatives considered

- **A single local password.** Protects against someone else at the computer, but adds a password that
  can't be recovered without email, for little gain over the OS account.
- **Keeping accounts for a cloud server only.** Keeps two products' worth of code (sessions, cookies,
  email flows, roles, deployment) for an option this app doesn't need.
