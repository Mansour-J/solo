#!/usr/bin/env node
// Runs the API locally (cargo run) with settings derived from the repository's .env file, adjusted
// for running outside Docker: the database and Mailpit on localhost, links pointing at the Vite dev
// server, readable logs. Values already in the environment win.
import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { delimiter, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('..', import.meta.url));
const envPath = join(root, '.env');
const fromFile = {};
if (existsSync(envPath)) {
  for (const line of readFileSync(envPath, 'utf8').split('\n')) {
    const match = /^\s*([A-Z0-9_]+)\s*=\s*(.*)\s*$/.exec(line);
    if (match) fromFile[match[1]] = match[2];
  }
} else {
  console.warn('No .env file: copy environment.example to .env (or run infra/scripts/setup.sh).');
}

const env = { ...fromFile, ...process.env };
const db = env.POSTGRES_DB || 'solo';
const port = env.POSTGRES_PORT || '5432';
const password = env.APP_DB_PASSWORD || 'change-me-app-password';
const local = {
  DATABASE_URL: `postgres://solo_app:${encodeURIComponent(password)}@localhost:${port}/${db}`,
  APP_URL: 'http://localhost:5173',
  SMTP_HOST: 'localhost',
  SMTP_PORT: env.MAILPIT_SMTP_PORT || '1025',
  STORAGE_LOCAL_PATH: join(root, 'data', 'storage'),
  LOG_FORMAT: 'pretty',
  TRUST_PROXY: 'false',
};
const finalEnv = { ...fromFile, ...local, ...process.env };
// Cargo's bin directory first. Windows separates PATH with ';' and names the variable 'Path'.
const pathKey = Object.keys(finalEnv).find((key) => key.toUpperCase() === 'PATH') ?? 'PATH';
finalEnv[pathKey] = [join(homedir(), '.cargo', 'bin'), finalEnv[pathKey]].filter(Boolean).join(delimiter);

const child = spawn('cargo', ['run', '--manifest-path', join(root, 'services/api/Cargo.toml')], { env: finalEnv, stdio: 'inherit' });
child.on('exit', (code) => process.exit(code ?? 0));
