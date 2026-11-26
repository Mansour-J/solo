#!/usr/bin/env node
// Coverage of the desktop app's update code (checking for, downloading, verifying and installing updates,
// ADR 0015), which must always work: a broken updater can't deliver its own fix.
//
//   node scripts/updates-coverage.mjs   (npm run desktop:coverage:updates)
//
// Runs `apps/client/src-tauri/tests/updates.rs` under cargo-llvm-cov and fails below 99% of the lines in
// `src/updates/mod.rs`. Two things aren't counted:
// - `src/updates/system.rs`: stopping the local database, running the installer and restarting, which
//   would replace or end the test program; it's checked by updating a real installed app.
// - `#[tauri::command]` lines: the macro leaves a closure there that nothing calls (its only region is
//   one character wide), so it reads as uncovered whatever the tests do. The commands themselves are
//   called through IPC by the tests.
//
// The client side (Settings › Updates and the update calls in services/platform.ts) has its own threshold
// in apps/client/vite.config.ts (`npm run coverage -w @solo/client`).
//
// Needs cargo-llvm-cov (`cargo install cargo-llvm-cov`, `rustup component add llvm-tools-preview`).
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const THRESHOLD = 99;
const manifest = join('apps', 'client', 'src-tauri', 'Cargo.toml');
const measured = /[\\/]src[\\/]updates[\\/]mod\.rs$/;
const output = join(mkdtempSync(join(tmpdir(), 'solo-updates-coverage-')), 'coverage.json');

const run = spawnSync('cargo', ['llvm-cov', '--manifest-path', manifest, '--test', 'updates', '--json', '--output-path', output], {
  stdio: 'inherit',
});
if (run.status !== 0) process.exit(run.status ?? 1);

const report = JSON.parse(readFileSync(output, 'utf8'));
const file = report.data[0].files.find((f) => measured.test(f.filename));
if (!file) {
  console.error('No coverage for src/updates/mod.rs: did the update tests run?');
  process.exit(1);
}

const source = readFileSync(file.filename, 'utf8').split(/\r?\n/);
// Segments are [line, column, count, hasCount, isRegionEntry, isGapRegion]. A line is code when a region
// starts on it, and covered when any of them ran.
const lines = new Map();
for (const [line, , count, hasCount, isEntry, isGap] of file.segments) {
  if (!hasCount || !isEntry || isGap) continue;
  if (source[line - 1]?.trim() === '#[tauri::command]') continue;
  lines.set(line, (lines.get(line) ?? false) || count > 0);
}
const missed = [...lines].filter(([, covered]) => !covered).map(([line]) => line);
const percent = (100 * (lines.size - missed.length)) / lines.size;

console.log(`\nUpdate code (src/updates/mod.rs): ${percent.toFixed(2)}% of ${lines.size} lines covered (minimum ${THRESHOLD}%).`);
for (const line of missed) console.log(`  not covered: line ${line}: ${source[line - 1].trim()}`);
process.exit(percent >= THRESHOLD ? 0 : 1);
