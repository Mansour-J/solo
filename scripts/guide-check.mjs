#!/usr/bin/env node
// Keeps the in-app guide (apps/client/src/features/guide/content.ts) in step with the screens.
//
//   node scripts/guide-check.mjs hook    Claude Code PreToolUse hook (.claude/settings.json): before a
//                                        `git commit`, if staged changes touch what users see (client
//                                        screens, navigation, routes) but not the guide, it stops the
//                                        commit (exit 2) and says what to do.
//   node scripts/guide-check.mjs status  The same check on the staged files, for people.
//
// A commit that changes no user-visible behaviour (a refactor, a test, a style fix) says so with a line
// `Guide: unchanged` in its message. The guide's own test (guide.test.tsx) then checks the content:
// every screen and Year end tab is linked, and every **label** exists in the app.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';

const GUIDE = 'apps/client/src/features/guide/content.ts';
const OPT_OUT = /Guide:\s*unchanged/i;

/** Files whose changes users can see: screens and their copy, navigation and routes. */
function userVisible(file) {
  if (!file.startsWith('apps/client/src/')) return false;
  if (/\.test\.tsx?$/.test(file) || file.startsWith('apps/client/src/test/')) return false;
  if (file.startsWith('apps/client/src/features/guide/')) return false;
  return file.endsWith('.tsx') || /\/(nav|router|routes)\.tsx?$/.test(file);
}

function staged() {
  return execFileSync('git', ['diff', '--cached', '--name-only'], { encoding: 'utf8' })
    .split('\n')
    .map((f) => f.trim())
    .filter(Boolean);
}

/** The user-visible files staged without a guide change, or [] when all is well. */
function unguided() {
  const files = staged();
  if (files.includes(GUIDE)) return [];
  return files.filter(userVisible);
}

const message = (files) =>
  `These staged changes touch screens users see, but the guide (${GUIDE}) isn't staged:\n` +
  files.map((f) => `  - ${f}`).join('\n') +
  `\nUpdate the guide in the same commit (see the update-guide skill): new or renamed buttons, fields, tabs and ` +
  `screens, with the app's own words in **bold**, then run the guide test. If nothing users see has changed ` +
  `(a refactor or a fix with the same labels), add a line "Guide: unchanged" to the commit message.\n`;

function hook() {
  let input;
  try {
    input = JSON.parse(fs.readFileSync(0, 'utf8') || '{}');
  } catch {
    return;
  }
  const command = String(input.tool_input?.command ?? '');
  // A git commit command, possibly after environment assignments or `git -c key=value`.
  const gitCommit = /(?:^|[;&|(])\s*(?:[A-Z_][A-Z0-9_]*=(?:'[^']*'|"[^"]*"|\S+)\s+)*git(?:\s+-c\s+(?:"[^"]*"|'[^']*'|\S+))*\s+commit\b/m;
  if (!gitCommit.test(command) || OPT_OUT.test(command)) return;
  let files;
  try {
    // `git commit -a` stages tracked changes as it commits: count them too.
    if (/\scommit\b[^\n]*\s-(?:a|[a-zA-Z]*a[a-zA-Z]*)\b/.test(command)) {
      const tracked = execFileSync('git', ['diff', '--name-only'], { encoding: 'utf8' }).split('\n').filter(Boolean);
      const all = [...new Set([...staged(), ...tracked])];
      files = all.includes(GUIDE) ? [] : all.filter(userVisible);
    } else {
      files = unguided();
    }
  } catch {
    return;
  }
  if (files.length === 0) return;
  process.stderr.write(message(files));
  process.exit(2);
}

const [command = 'status'] = process.argv.slice(2);
if (command === 'hook') hook();
else if (command === 'status') {
  const files = unguided();
  if (files.length === 0) console.log('The guide is staged with any user-visible changes (or there are none).');
  else {
    process.stdout.write(message(files));
    process.exit(1);
  }
} else {
  console.error('Usage: node scripts/guide-check.mjs [hook|status]');
  process.exit(2);
}
