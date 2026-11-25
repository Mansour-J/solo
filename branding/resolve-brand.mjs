// Resolves the product identity for build tooling (Vite, brand-sync): branding/brand.json with
// APP_* environment overrides applied. The API applies the same overrides at runtime
// (services/api/src/config/brand.rs), so both sides always agree.
import { readFileSync } from 'node:fs';

const brandFile = new URL('./brand.json', import.meta.url);

/** @param {Record<string, string | undefined>} env */
export function resolveBrand(env = process.env) {
  const file = JSON.parse(readFileSync(brandFile, 'utf8'));
  const pick = (key, fallback) => {
    const value = env[key]?.trim();
    return value ? value : fallback;
  };
  const brand = {
    name: pick('APP_NAME', file.name),
    shortName: pick('APP_SHORT_NAME', file.shortName),
    description: pick('APP_DESCRIPTION', file.description),
    tagline: pick('APP_TAGLINE', file.tagline),
    primaryColor: pick('APP_PRIMARY_COLOR', file.colors.primary),
    secondaryColor: pick('APP_SECONDARY_COLOR', file.colors.secondary),
    supportEmail: pick('APP_SUPPORT_EMAIL', file.email.supportEmail),
    websiteUrl: pick('APP_WEBSITE_URL', file.urls.website),
    termsUrl: pick('APP_TERMS_URL', file.legal.termsUrl),
    privacyUrl: pick('APP_PRIVACY_URL', file.legal.privacyUrl),
    copyright: pick('APP_COPYRIGHT', file.copyright),
    desktopIdentifier: pick('APP_DESKTOP_IDENTIFIER', file.identifiers.desktop),
  };
  for (const key of ['primaryColor', 'secondaryColor']) {
    if (!/^#[0-9a-fA-F]{6}$/.test(brand[key])) throw new Error(`Brand ${key} must be a #rrggbb colour, got ${brand[key]}`);
  }
  return brand;
}

export const brandAssetsDir = new URL('./assets/', import.meta.url);
