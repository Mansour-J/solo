/// <reference types="vitest/config" />
import { readFileSync } from 'node:fs';
import { fileURLToPath, URL } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig, loadEnv, type Plugin } from 'vite';
import { brandAssetsDir, resolveBrand } from '../../branding/resolve-brand.mjs';

const repoRoot = fileURLToPath(new URL('../..', import.meta.url));
const appVersion = (JSON.parse(readFileSync(new URL('../../package.json', import.meta.url), 'utf8')) as { version: string }).version;
const buildId = `${appVersion}+${process.env.GITHUB_SHA?.slice(0, 12) ?? Date.now().toString(36)}`;

interface ResolvedBrand {
  name: string;
  shortName: string;
  description: string;
  primaryColor: string;
}

/** Writes the product identity into index.html, so the first paint is already branded. */
function brandHtml(brand: ResolvedBrand): Plugin {
  const escape = (value: string) => value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/"/g, '&quot;');
  return {
    name: 'brand-html',
    transformIndexHtml: (html) =>
      html
        .replaceAll('%BRAND_NAME%', escape(brand.name))
        .replaceAll('%BRAND_DESCRIPTION%', escape(brand.description))
        .replaceAll('%BRAND_PRIMARY%', escape(brand.primaryColor)),
  };
}

/** Emits version.json so a running web app can tell that a newer build was deployed. */
function versionManifest(): Plugin {
  return {
    name: 'version-manifest',
    apply: 'build',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'version.json',
        source: `${JSON.stringify({ version: appVersion, buildId, builtAt: new Date().toISOString() }, null, 2)}\n`,
      });
    },
  };
}

export default defineConfig(({ mode }) => {
  // Branding (APP_*) and public client settings (VITE_*) come from the repository-root env file and
  // the process environment, the same sources the API and Docker Compose use.
  const env = loadEnv(mode, repoRoot, ['APP_', 'VITE_']);
  const brand = resolveBrand({ ...env, ...process.env }) as ResolvedBrand;
  const apiProxyTarget = env.VITE_DEV_API_PROXY || 'http://localhost:8000';
  const host = process.env.TAURI_DEV_HOST;

  return {
    plugins: [react(), brandHtml(brand), versionManifest()],
    envDir: repoRoot,
    envPrefix: ['VITE_', 'TAURI_ENV_'],
    // Brand assets (favicon, logos) are served from the site root.
    publicDir: fileURLToPath(brandAssetsDir),
    define: {
      __BRAND__: JSON.stringify(brand),
      __APP_VERSION__: JSON.stringify(appVersion),
      __BUILD_ID__: JSON.stringify(buildId),
    },
    resolve: {
      alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
    },
    clearScreen: false,
    server: {
      port: 5173,
      strictPort: true,
      host: host || false,
      hmr: host ? { protocol: 'ws', host, port: 5174 } : undefined,
      watch: { ignored: ['**/src-tauri/**'] },
      // Same-origin API in development, as in production (the web container proxies /api).
      proxy: {
        '/api': { target: apiProxyTarget, changeOrigin: false, xfwd: true },
        '/health': { target: apiProxyTarget },
      },
      fs: { allow: [repoRoot] },
    },
    preview: { port: 4173, strictPort: true, proxy: { '/api': { target: apiProxyTarget } } },
    build: {
      // Tauri uses Chromium on Windows and WebKit on macOS/Linux.
      target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome111' : 'safari16',
      sourcemap: !!process.env.TAURI_ENV_DEBUG,
      chunkSizeWarningLimit: 1500,
    },
    test: {
      globals: true,
      environment: 'jsdom',
      setupFiles: ['./src/test/setup.ts'],
      include: ['src/**/*.test.{ts,tsx}'],
      css: false,
      testTimeout: 20000,
      // `npm run coverage`: the update code (Settings › Updates and the calls into the desktop shell) must
      // always work, since a broken updater can't deliver its own fix. The Rust side is measured by
      // scripts/updates-coverage.mjs.
      coverage: {
        provider: 'v8',
        include: ['src/features/updates/**/*.{ts,tsx}', 'src/services/platform.ts'],
        exclude: ['**/*.test.{ts,tsx}'],
        reporter: ['text'],
        thresholds: { lines: 99, statements: 99, functions: 99, branches: 99 },
      },
    },
  };
});
