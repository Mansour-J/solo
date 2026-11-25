#!/usr/bin/env node
// Keeps the published copy of the backlog (a private claude.ai artifact) in step with
// docs/backlog/backlog.html.
//
//   npm run backlog:publish -- export      write the copy to publish: dist/backlog/solo-backlog.html
//   npm run backlog:publish -- published   record that the current page is what's published
//   npm run backlog:publish -- status      say whether the committed page differs from what's published
//   node scripts/backlog-publish.mjs hook  Claude Code PostToolUse hook (.claude/settings.json): after a
//                                          push to main, tells Claude to republish when the page changed
//
// Only Claude's Artifact tool can publish an artifact, so this script prepares the file and the hook
// reminds Claude; the steps are in .claude/skills/backlog/SKILL.md ("Republishing").
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ARTIFACT_URL = 'https://claude.ai/artifact/1YuYxV3QqVkS9jVMFBnAy6';
const REPO_BLOB = 'https://github.com/Mansour-J/solo/blob/main/';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const source = path.join(root, 'docs/backlog/backlog.html');
const exportDir = path.join(root, 'dist/backlog');
const exportFile = path.join(exportDir, 'solo-backlog.html');
const publishedFile = path.join(exportDir, 'published.sha256');

// Line endings and surrounding whitespace are ignored, so a Windows checkout hashes like `git show`.
const hash = (text) => createHash('sha256').update(text.replace(/\r\n/g, '\n').trim()).digest('hex');
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
const published = () => (fs.existsSync(publishedFile) ? fs.readFileSync(publishedFile, 'utf8').trim() : '');
const committedHash = () => hash(git('show', 'HEAD:docs/backlog/backlog.html'));

/** The page as the artifact viewer wants it: no document skeleton, and repository links on GitHub. */
function exportPage() {
  const page = fs
    .readFileSync(source, 'utf8')
    .replace(/^\s*<!doctype html>\s*<html[^>]*>\s*<head>\s*/i, '')
    .replace(/<meta charset="utf-8" \/>\s*/, '')
    .replace(/<meta name="viewport"[^>]*\/>\s*/, '')
    .replace(/<!--\s*Solo's backlog:[\s\S]*?-->\s*/, '')
    .replace(/\s*<\/head>\s*<body>/, '')
    .replace(/\s*<\/body>\s*<\/html>\s*$/, '\n')
    .replaceAll('](../../', `](${REPO_BLOB}`)
    .replaceAll('](../', `](${REPO_BLOB}docs/`);
  if (/<html|<body|\]\(\.\.\//i.test(page)) throw new Error('export: the page skeleton or a relative link survived');
  fs.mkdirSync(exportDir, { recursive: true });
  fs.writeFileSync(exportFile, page);
  return path.relative(root, exportFile).replaceAll('\\', '/');
}

const instructions = (file) =>
  `Republish the backlog artifact: read ${ARTIFACT_URL} with the Artifact tool (action "read"), publish ${file} ` +
  `with url ${ARTIFACT_URL}, then run \`npm run backlog:publish -- published\`. See the backlog skill.`;

/** PostToolUse hook: exit 2 shows stderr to Claude after a push to main that left the artifact stale. */
function hook() {
  let input;
  try {
    input = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
  } catch {
    return;
  }
  const command = String(input.tool_input?.command ?? '');
  // A git push command, possibly after environment assignments or `git -c key=value`, not text that
  // merely mentions one (a commit message, an echo).
  const gitPush = /(?:^|[;&|(])\s*(?:[A-Z_][A-Z0-9_]*=(?:'[^']*'|"[^"]*"|\S+)\s+)*git(?:\s+-c\s+(?:"[^"]*"|'[^']*'|\S+))*\s+push\b/m;
  if (!gitPush.test(command)) return;
  try {
    const onMain = /\bmain\b/.test(command) || git('rev-parse', '--abbrev-ref', 'HEAD').trim() === 'main';
    if (!onMain || published() === committedHash()) return;
  } catch {
    return;
  }
  const file = exportPage();
  process.stderr.write(`The backlog pushed to main differs from the published artifact. ${instructions(file)}\n`);
  process.exit(2);
}

const [command = 'status'] = process.argv.slice(2);
try {
  if (command === 'hook') hook();
  else if (command === 'export') {
    const file = exportPage();
    console.log(`Wrote ${file}.\n${instructions(file)}`);
  } else if (command === 'published') {
    fs.mkdirSync(exportDir, { recursive: true });
    fs.writeFileSync(publishedFile, `${hash(fs.readFileSync(source, 'utf8'))}\n`);
    console.log(`Recorded docs/backlog/backlog.html as published at ${ARTIFACT_URL}`);
  } else if (command === 'status') {
    const current = published() === committedHash();
    console.log(current ? `Up to date: ${ARTIFACT_URL}` : `Out of date (or never recorded): ${ARTIFACT_URL}`);
    if (!current) process.exitCode = 1;
  } else throw new Error(`Unknown command "${command}". Use export, published, status or hook.`);
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
