#!/usr/bin/env node
// API test coverage, measured separately for each kind of test (docs/testing.md):
//
//   node scripts/api-coverage.mjs unit         unit tests (--lib) over the domain modules
//   node scripts/api-coverage.mjs integration  integration tests (tests/*.rs, no unit tests) over the whole crate
//
// Each fails below 90% of lines. Needs cargo-llvm-cov (`cargo install cargo-llvm-cov`,
// `rustup component add llvm-tools-preview`); integration needs the test database (TEST_DATABASE_URL).
import { spawnSync } from 'node:child_process';
import { readdirSync } from 'node:fs';
import { join } from 'node:path';

const THRESHOLD = 90;
const manifest = join('services', 'api', 'Cargo.toml');
const kind = process.argv[2];

const args = ['llvm-cov', '--manifest-path', manifest, '--summary-only', '--fail-under-lines', String(THRESHOLD)];
if (kind === 'unit') {
  // Unit tests cover pure logic; everything else is exercised by integration tests.
  args.push(
    '--lib',
    '--ignore-filename-regex',
    String.raw`src[\\/](api|auth|config|middleware|jobs|repositories|services)[\\/]|src[\\/](main|db|error|state|lib|migrations)\.rs`,
  );
} else if (kind === 'integration') {
  const tests = readdirSync(join('services', 'api', 'tests')).filter((f) => f.endsWith('.rs'));
  for (const file of tests) args.push('--test', file.replace(/\.rs$/, ''));
} else {
  console.error('Usage: node scripts/api-coverage.mjs unit|integration');
  process.exit(2);
}

const result = spawnSync('cargo', args, { stdio: 'inherit' });
process.exit(result.status ?? 1);
