# 0005 — OpenAPI generated from the API, typed client generated from OpenAPI

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

The client must stay in step with the API. Hermes Hub hand-wrote a TypeScript contract; Solo's API
is Rust, so a hand-written TypeScript contract would duplicate every type.

## Decision

- Handlers are annotated with `utoipa`; `utoipa-axum` collects them into the OpenAPI document served
  at `/api/openapi.json` with a reference UI at `/api/docs`. Operation IDs are `<tag>_<handler>`.
- `npm run api:client` runs `solo-api openapi`, writes `packages/api-client/openapi.json` and
  generates `schema.d.ts` with `openapi-typescript`. The client uses `openapi-fetch`, so paths,
  parameters and bodies are type-checked.
- Both generated files are committed; CI regenerates them and fails on any difference.

## Consequences

- An API change that breaks the client fails `npm run typecheck`.
- JSON field names are the Rust (snake_case) names, used as-is in TypeScript.

## Alternatives considered

- **Hand-written TypeScript types:** drift.
- **ts-rs / specta (types without OpenAPI):** no API documentation, no path typing.
