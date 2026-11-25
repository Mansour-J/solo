#!/usr/bin/env node
// Applies the product identity (branding/brand.json + APP_* overrides) to files that cannot read it at
// runtime: the desktop app's Tauri configuration and icons. The web app and API read the brand
// themselves. CI runs `npm run brand:sync -- --check` and fails if the committed files are stale.
//
//   npm run brand:sync             update tauri.conf.json
//   npm run brand:sync -- --icons  also regenerate desktop icons from branding/assets/app-icon.svg
//   npm run brand:sync -- --check  verify only
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolveBrand } from '../branding/resolve-brand.mjs';

const root = fileURLToPath(new URL('..', import.meta.url));
const tauriConfPath = `${root}/apps/client/src-tauri/tauri.conf.json`;
const args = new Set(process.argv.slice(2));
const brand = resolveBrand(process.env);

const conf = JSON.parse(readFileSync(tauriConfPath, 'utf8'));
conf.productName = brand.name;
conf.identifier = brand.desktopIdentifier;
conf.app.windows[0].title = brand.name;
conf.bundle.shortDescription = brand.description;
conf.bundle.longDescription = `${brand.description}. ${brand.tagline}`;
conf.bundle.copyright = brand.copyright;
const next = `${JSON.stringify(conf, null, 2)}\n`;

if (args.has('--check')) {
  if (readFileSync(tauriConfPath, 'utf8') !== next) {
    console.error('apps/client/src-tauri/tauri.conf.json is out of date with branding/brand.json. Run: npm run brand:sync');
    process.exit(1);
  }
  console.log('Desktop configuration matches the brand.');
} else {
  writeFileSync(tauriConfPath, next);
  console.log(`Desktop configuration updated for “${brand.name}” (${brand.desktopIdentifier}).`);
  if (args.has('--icons')) {
    execFileSync('npx', ['tauri', 'icon', `${root}/branding/assets/app-icon.svg`, '-o', `${root}/apps/client/src-tauri/icons`], {
      cwd: `${root}/apps/client`,
      stdio: 'inherit',
    });
  }
}
