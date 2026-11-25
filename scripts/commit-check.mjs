#!/usr/bin/env node
// Checks commit messages follow Conventional Commits (https://www.conventionalcommits.org), which
// release-please turns into the next semantic version and the CHANGELOG.md section.
//
//   node scripts/commit-check.mjs --file .git/COMMIT_EDITMSG   the commit-msg hook (.githooks/commit-msg)
//   node scripts/commit-check.mjs --range origin/main..HEAD    every commit in a range (CI)
//
// <type>[(scope)][!]: <description>, e.g. "feat(invoices): email a statement" or "fix!: …".
// feat → minor version, fix/perf/deps → patch, "!" or a "BREAKING CHANGE:" footer → major (minor
// while the version is 0.x). Other types don't make a release on their own and stay out of the changelog.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

const TYPES = ['feat', 'fix', 'perf', 'revert', 'deps', 'docs', 'refactor', 'test', 'build', 'ci', 'style', 'chore'];
const HEADER = new RegExp(`^(${TYPES.join('|')})(\\([a-z0-9][a-z0-9-]*\\))?!?: \\S.*$`);
const EXEMPT = /^(Merge |Revert "|fixup! |squash! |amend! )/;
const MAX = 100;

/** Problems with one commit message (empty when it's fine). */
function check(message) {
  const header = message.split('\n').find((line) => line.trim() && !line.startsWith('#')) ?? '';
  if (EXEMPT.test(header)) return [];
  const problems = [];
  if (!HEADER.test(header)) {
    problems.push(`"${header}" isn't <type>[(scope)][!]: <description> (types: ${TYPES.join(', ')})`);
  }
  if (header.length > MAX) problems.push(`the first line is ${header.length} characters (at most ${MAX})`);
  return problems;
}

function report(label, problems) {
  for (const p of problems) console.error(`✗ ${label}: ${p}`);
  return problems.length;
}

const args = process.argv.slice(2);
const value = (flag) => args[args.indexOf(flag) + 1];
let failures = 0;
if (args.includes('--file')) {
  failures += report('commit message', check(readFileSync(value('--file'), 'utf8')));
} else if (args.includes('--range')) {
  const log = execFileSync('git', ['log', '--no-merges', '--format=%H%x00%B%x1e', value('--range')], { encoding: 'utf8' });
  for (const entry of log.split('\x1e').filter((e) => e.trim())) {
    const [hash, body] = entry.trim().split('\0');
    failures += report(hash.slice(0, 7), check(body));
  }
} else {
  console.error('Usage: commit-check.mjs --file <message file> | --range <a..b>');
  process.exit(2);
}
if (failures) {
  console.error('\nUse Conventional Commits: e.g. "feat(invoices): email a statement", "fix: …", "docs: …" (CONTRIBUTING.md).');
  process.exit(1);
}
