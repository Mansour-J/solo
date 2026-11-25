# Branding

The product identity is configuration, not code.

## Where it lives

| File | Holds |
|---|---|
| `branding/brand.json` | Name, short name, description, tagline, colours, asset paths, email sender and support address, website and app URLs, terms/privacy URLs, desktop/mobile identifiers, copyright |
| `branding/assets/` | `logo-mark.svg` (app header, start-up screen), `logo.svg`, `favicon.svg`, `app-icon.svg` (desktop/mobile icons) |
| `branding/resolve-brand.mjs` | Reads `brand.json` + `APP_*` overrides for build tooling |
| `services/api/src/config/brand.rs` | The same for the API (embedded at compile time, overrides at runtime) |

## Overriding per deployment

Set any of these in `.env` (empty = use `brand.json`):

`APP_NAME`, `APP_SHORT_NAME`, `APP_DESCRIPTION`, `APP_TAGLINE`, `APP_PRIMARY_COLOR`,
`APP_SECONDARY_COLOR`, `APP_SUPPORT_EMAIL`, `APP_WEBSITE_URL`, `APP_URL`, `APP_EMAIL_SENDER_NAME`,
`APP_EMAIL_FROM`, `APP_TERMS_URL`, `APP_PRIVACY_URL`, `APP_COPYRIGHT`, `APP_DESKTOP_IDENTIFIER`.

The API picks them up on restart. The app is branded at build time: rebuild it (`npm run desktop:build`)
after `npm run brand:sync`.

## Renaming the product

1. Edit `branding/brand.json` (and replace the SVGs in `branding/assets/` if the logo changes).
2. `npm run brand:sync -- --icons` updates the desktop configuration and regenerates its icons.
3. `npm run api:client` (the OpenAPI title includes the name) and commit.

Nothing else changes: components use `BRAND` from `apps/client/src/branding/brand.ts`, emails use the
API's `Brand`, and the theme palette is generated from the brand colour. Technical identifiers (crate
and npm package names, database schema and role, cookie and header names) intentionally stay as they
are; changing the desktop identifier makes the OS treat the app as a new application.
