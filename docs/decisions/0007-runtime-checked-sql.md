# 0007 — Runtime-checked SQL with integration tests

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

SQLx can check queries at compile time (`query!`), which needs a live database or committed
`.sqlx` offline metadata kept in step with migrations that Flyway (not SQLx) applies.

## Decision

Use runtime-checked queries (`query_as` with `FromRow`), with SQL kept in repository modules and
column lists defined once as compile-time string literals (SQLx 0.9 rejects dynamic SQL strings).
Every repository is exercised by integration tests against a real, Flyway-migrated PostgreSQL in CI.

## Consequences

- No offline metadata to maintain; the API builds without a database.
- A typo in SQL is caught by tests rather than the compiler, so repository functions need test coverage.

## Alternatives considered

- **`query!` macros with offline mode:** stronger compile-time guarantees; revisit if the team grows
  and the extra `cargo sqlx prepare` step is acceptable.
