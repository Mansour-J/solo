# 0004 — Sessions and tokens

- **Status:** Superseded by 0016 (there is no sign-in)
- **Date:** 2026-11-20

## Context

The same API serves browsers and a desktop app (and later mobile apps). Sessions must be revocable
(sign out a device, password changes), tokens must not be readable by injected scripts, and stolen
refresh tokens should be detectable.

## Decision

- **Opaque random tokens**, hashed with SHA-256 in `user_sessions`: a 15-minute access token and a
  refresh token that **rotates on every use**; reuse of a rotated token outside a 60-second grace
  window revokes the session.
- **Browsers:** tokens only in `HttpOnly` cookies (`SameSite=Lax` access cookie scoped to `/api`,
  `SameSite=Strict` refresh cookie scoped to `/api/v1/auth`), `Secure` over HTTPS. CSRF is blocked by
  requiring the custom `x-client-kind` header on state-changing requests.
- **Desktop/mobile:** tokens in the response body; refresh token in the OS keychain.
- Error codes distinguish `TOKEN_EXPIRED` (client refreshes and retries transparently) from
  `SESSION_EXPIRED` (sign in again).

## Consequences

- Each authenticated request does one indexed lookup; revocation is immediate.
- Sessions survive API restarts and scale across instances (state is in PostgreSQL).
- The in-memory auth rate limiter is per instance; move it to a shared store before scaling out.

## Alternatives considered

- **JWT access tokens:** no per-request lookup, but no immediate revocation without a deny-list.
- **Tokens in localStorage:** readable by any XSS.
- **Cookies for desktop too:** the webview origin differs from the API origin, making them
  third-party cookies, which webviews increasingly block.
