#!/usr/bin/env node
// Sets one version everywhere: npm packages, the lockfile, and the three Cargo crates.
//   npm run release:version -- 0.2.0
// Then: commit "Release v0.2.0", tag v0.2.0, push the tag (the Release workflow does the rest).
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? '')) {
  console.error('Usage: npm run release:version -- X.Y.Z');
  process.exit(1);
}
const root = fileURLToPath(new URL('..', import.meta.url));

for (const file of ['package.json', 'apps/client/package.json', 'packages/api-client/package.json']) {
  const path = `${root}/${file}`;
  const json = JSON.parse(readFileSync(path, 'utf8'));
  json.version = version;
  // Workspace packages depend on each other by exact version; keep those references in step, or npm
  // looks for the old version on the public registry.
  for (const field of ['dependencies', 'devDependencies']) {
    for (const name of Object.keys(json[field] ?? {})) {
      if (name.startsWith('@solo/')) json[field][name] = version;
    }
  }
  writeFileSync(path, `${JSON.stringify(json, null, 2)}\n`);
}
// On Windows npm is a .cmd script, which needs a shell to run.
execFileSync('npm', ['install', '--package-lock-only', '--no-audit', '--no-fund'], {
  cwd: root,
  stdio: 'inherit',
  shell: process.platform === 'win32',
});

// In dependency order: the desktop app's lockfile includes the local server, which includes the API.
for (const crate of ['services/api', 'services/local', 'apps/client/src-tauri']) {
  const path = `${root}/${crate}/Cargo.toml`;
  writeFileSync(path, readFileSync(path, 'utf8').replace(/^version = ".*"$/m, `version = "${version}"`));
  execFileSync('cargo', ['update', '--workspace', '--manifest-path', path], { cwd: root, stdio: 'inherit' });
}
console.log(
  `Version set to ${version}. Next: git commit -am "Release v${version}" && git tag -a v${version} -m "Release v${version}" && git push origin HEAD v${version}`,
);
