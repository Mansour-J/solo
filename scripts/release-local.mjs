#!/usr/bin/env node
// Builds the Windows installers on this machine and publishes them as a GitHub release, instead of the
// Release workflow (GitHub Actions minutes cost money on a private repository; see CLAUDE.md).
//
//   npm run release:local                 # build, tag vX.Y.Z (from package.json), push the tag, publish
//   npm run release:local -- --dry-run    # build and list what would be published; no tag, no upload
//
// Before: `npm run release:version -- X.Y.Z`, commit "Release vX.Y.Z", push main, and pass the local
// gate (`/check full`). Pushing uses your git setup (for Solo: GIT_SSH_COMMAND with the personal key).
//
// Update packages are signed only when TAURI_SIGNING_PRIVATE_KEY (the key's contents or path) and
// TAURI_SIGNING_PRIVATE_KEY_PASSWORD are set; then `latest.json` is uploaded too. Without them the
// installers still build, and apps simply aren't offered the update (the feed isn't public yet).
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const dryRun = process.argv.includes('--dry-run');
const run = (cmd, args, options = {}) =>
  // npm is a .cmd script on Windows and needs a shell; nothing passed to it contains spaces.
  execFileSync(cmd, args, { cwd: root, encoding: 'utf8', shell: cmd === 'npm' && process.platform === 'win32', ...options });
const out = (cmd, args) => run(cmd, args, { stdio: ['ignore', 'pipe', 'inherit'] }).trim();
const fail = (message) => {
  console.error(`release:local: ${message}`);
  process.exit(1);
};

if (process.platform !== 'win32') fail('builds the Windows installers, so run it on Windows.');

// ─── Checks ─────────────────────────────────────────────────────────────────
const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const tag = `v${version}`;
for (const manifest of ['services/api/Cargo.toml', 'services/local/Cargo.toml', 'apps/client/src-tauri/Cargo.toml']) {
  const cargo = /^version = "(.*)"/m.exec(readFileSync(join(root, manifest), 'utf8'))?.[1];
  if (cargo !== version) fail(`${manifest} is ${cargo}, package.json is ${version}: run npm run release:version -- ${version}.`);
}
if (!dryRun) {
  if (out('git', ['branch', '--show-current']) !== 'main') fail('release from main.');
  if (out('git', ['status', '--porcelain'])) fail('the working tree has changes: commit or stash them first.');
  run('git', ['fetch', 'origin', 'main', '--tags'], { stdio: 'inherit' });
  if (out('git', ['rev-parse', 'HEAD']) !== out('git', ['rev-parse', 'origin/main'])) fail('push main first (HEAD is not origin/main).');
  if (out('git', ['ls-remote', '--tags', 'origin', tag])) fail(`${tag} is already on GitHub.`);
}

// ─── Build ──────────────────────────────────────────────────────────────────
const signed = Boolean(process.env.TAURI_SIGNING_PRIVATE_KEY);
const buildArgs = ['run', 'desktop:build', '--', '--bundles', 'nsis,msi'];
if (signed) buildArgs.push('--config', 'src-tauri/tauri.updater.conf.json');
console.log(`Building Solo ${version} installers${signed ? ' with signed update packages' : ''}…`);
run('npm', buildArgs, { stdio: 'inherit', env: { ...process.env, PATH: `${process.env.USERPROFILE}\\.cargo\\bin;${process.env.PATH}` } });

const bundle = join(root, 'apps/client/src-tauri/target/release/bundle');
const pick = (dir, test) =>
  existsSync(join(bundle, dir))
    ? readdirSync(join(bundle, dir))
        .filter((f) => f.includes(`_${version}_`) && test(f))
        .map((f) => join(bundle, dir, f))
    : [];
const nsis = pick('nsis', (f) => f.endsWith('-setup.exe'));
const msi = pick('msi', (f) => f.endsWith('.msi'));
if (nsis.length !== 1 || msi.length !== 1) fail(`expected one NSIS and one MSI installer for ${version} in ${bundle}.`);
const assets = [nsis[0], msi[0]];

// ─── Update feed (only with the signing key) ────────────────────────────────
const repo = out('gh', ['repo', 'view', '--json', 'nameWithOwner', '-q', '.nameWithOwner']);
if (signed) {
  const signature = `${nsis[0]}.sig`;
  if (!existsSync(signature)) fail(`no update signature next to ${nsis[0]}.`);
  const file = nsis[0].split(/[\\/]/).pop();
  const latest = {
    version,
    notes: `Solo ${version}`,
    pub_date: new Date().toISOString(),
    platforms: {
      'windows-x86_64': {
        signature: readFileSync(signature, 'utf8').trim(),
        url: `https://github.com/${repo}/releases/download/${tag}/${file}`,
      },
    },
  };
  const latestPath = join(tmpdir(), 'latest.json');
  writeFileSync(latestPath, `${JSON.stringify(latest, null, 2)}\n`);
  assets.push(signature, latestPath);
}

// ─── Notes ──────────────────────────────────────────────────────────────────
let previous = '';
try {
  previous = out('git', ['describe', '--tags', '--abbrev=0', 'HEAD^']);
} catch {
  // The first release: every commit.
}
const log = out('git', ['log', '--no-merges', '--pretty=- %s', previous ? `${previous}..HEAD` : 'HEAD'])
  .split('\n')
  .slice(0, 60);
const notes = [
  '## Changes',
  ...log,
  '',
  '## Upgrading',
  'Install the new version over the old one (or use Settings › Updates). Your books are kept and updated when it starts.',
  '',
  'Windows only: built on the maintainer’s machine (see docs/deployment/release-process.md).',
  '',
].join('\n');
const notesPath = join(tmpdir(), `solo-${tag}-notes.md`);
writeFileSync(notesPath, notes);

if (dryRun) {
  console.log(`\nDry run: would tag ${tag} and publish to ${repo}:`);
  for (const a of assets) console.log(`  ${a}`);
  console.log(`\n${notes}`);
  process.exit(0);
}

// ─── Tag and publish ────────────────────────────────────────────────────────
run('git', ['tag', '-a', tag, '-m', `Release ${tag}`], { stdio: 'inherit' });
run('git', ['push', 'origin', tag], { stdio: 'inherit' });
run('gh', ['release', 'create', tag, '--repo', repo, '--title', tag, '--notes-file', notesPath, '--latest', '--verify-tag', ...assets], {
  stdio: 'inherit',
});
console.log(`\nPublished ${tag}: ${out('gh', ['release', 'view', tag, '--repo', repo, '--json', 'url', '-q', '.url'])}`);
