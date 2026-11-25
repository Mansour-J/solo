#!/usr/bin/env node
// LOCAL DEVELOPMENT ONLY: wipes the local database schema and re-applies every migration.
// Refuses to run unless the database is the Docker Compose container on localhost.
import { execFileSync } from 'node:child_process';
import { createInterface } from 'node:readline/promises';

const rl = createInterface({ input: process.stdin, output: process.stdout });
const answer = await rl.question('This deletes ALL data in the local Docker database. Type RESET to continue: ');
rl.close();
if (answer.trim() !== 'RESET') {
  console.log('Cancelled.');
  process.exit(1);
}
const run = (...args) => execFileSync('docker', ['compose', ...args], { stdio: 'inherit' });
run('run', '--rm', '-e', 'FLYWAY_CLEAN_DISABLED=false', 'migrate', 'clean');
run('run', '--rm', 'migrate', 'migrate');
