#!/usr/bin/env node
// Regenerates the screenshots the README shows (docs/screenshots), with a fresh seeded test business
// whose records are all labelled "(test)". Needs the Vite dev server and an API (see scripts/snap.mjs).
//
//   npm run readme:screenshots
//
// Keep the list in step with the "Screenshots" section of README.md.
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const snap = fileURLToPath(new URL('./snap.mjs', import.meta.url));

/** [flags, shots] per run; the first run creates the business the others reuse. */
const RUNS = [
  [
    ['--fresh'],
    [
      'dashboard:/dashboard',
      'invoices:/invoices',
      'invoice:/invoices/{invoice}',
      'expenses:/expenses',
      'mileage:/mileage',
      'allocations:/allocations',
      'allocation-review:/allocations?review={allocation}',
      'reports:/reports',
      'statement:/reports?report=statement_of_account&customer={customer}',
      'insights:/insights',
      'year-end:/year-end',
      'assets:/year-end?tab=assets',
      'balance-sheet:/reports?report=balance_sheet',
    ],
  ],
  [['--dark'], ['dashboard:/dashboard', 'reports:/reports', 'insights:/insights']],
  [['--mobile'], ['dashboard:/dashboard', 'allocations:/allocations']],
];

for (const [flags, shots] of RUNS) {
  const result = spawnSync(process.execPath, [snap, ...flags, ...shots], {
    cwd: root,
    stdio: 'inherit',
    env: { ...process.env, SNAP_OUT_DIR: 'docs/screenshots' },
  });
  if (result.status !== 0) process.exit(result.status ?? 1);
}
console.log('README screenshots updated in docs/screenshots.');
