#!/usr/bin/env node
// Adds the demo business, "Fernbird Consulting (demo)", the same one the app offers on first run
// (apps/client/src/features/demo/demoData.ts): last financial year and this one so far, shaped like a
// contractor's. Every name and figure is invented. Needs Node 22.18 or later (it runs the TypeScript).
//
//   SEED_BASE_URL=http://127.0.0.1:<port> node scripts/seed-demo.mjs   straight to an API (e.g. solo-local)
//   node scripts/seed-demo.mjs                                         through Vite (http://localhost:5173)
//
// There is no sign-in: the business is added for the server's owner, next to their own businesses.
// A server that requires a key (the desktop app's own) needs SEED_API_KEY. There is one demo at a time:
// reset it from the app's demo banner.
import { createDemoBusiness, DEMO_NAME } from '../apps/client/src/features/demo/demoData.ts';

const base = (process.env.SEED_BASE_URL ?? 'http://localhost:5173').replace(/\/$/, '');
const NAME = process.env.SEED_DEMO_NAME ?? DEMO_NAME;

async function call(method, path, body) {
  const response = await fetch(`${base}/api/v1${path}`, {
    method,
    headers: {
      'content-type': 'application/json',
      'x-client-kind': 'web',
      ...(process.env.SEED_API_KEY ? { 'x-api-key': process.env.SEED_API_KEY } : {}),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = response.status === 204 ? null : await response.json().catch(() => null);
  if (!response.ok) throw new Error(`${method} ${path} failed (${response.status}): ${JSON.stringify(json)}`);
  return json;
}

const existing = await call('GET', '/businesses');
if (existing.some((b) => b.is_demo)) {
  console.log('There is already a demo business; nothing to do.');
  process.exit(0);
}
let shown = 0;
const id = await createDemoBusiness(
  call,
  (fraction) => {
    const percent = Math.floor(fraction * 10) * 10;
    if (percent > shown) console.log(`${(shown = percent)}%`);
  },
  NAME,
);
console.log(`Added ${NAME} (${id}). Choose it from the business switcher to see it.`);
