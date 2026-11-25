# 0006 — Central branding configuration

- **Status:** Accepted
- **Date:** 2026-11-20

## Context

The product was renamed once already (from its working name to Solo) and may be renamed again or
white-labelled. Renaming must not mean searching and replacing across the codebase.

## Decision

- `branding/brand.json` holds the identity: name, short name, description, tagline, colours, logo and
  icon assets, email sender and support address, website and app URLs, terms/privacy links, package
  identifiers and copyright.
- `APP_*` environment variables override any user-facing value per deployment.
- The API embeds `brand.json` and applies overrides at start-up (emails, OpenAPI title, `/api/v1/meta`).
- The web build injects the resolved brand (`__BRAND__`), writes it into `index.html`, serves the
  assets, and generates the Mantine palette from the brand colour.
- `npm run brand:sync` writes it into the Tauri configuration (and regenerates icons with `--icons`);
  CI fails if that configuration is stale.
- Technical names (crate and package names, database role and schema, header and cookie names) are
  deliberately not branded and don't change on a rename.

## Consequences

- Rebranding a deployment: set `APP_*` values and rebuild the web image. Rebranding the product:
  edit `brand.json` and assets, run `npm run brand:sync -- --icons`.

## Alternatives considered

- **Runtime-only branding fetched from the API:** the first paint and the HTML title would flash the
  default brand, and desktop bundles need the name at build time anyway.
