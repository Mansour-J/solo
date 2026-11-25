// Platform detection and native capabilities (Tauri), with web fallbacks.

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** How this client identifies itself to the API (`x-client-kind`). */
export const CLIENT_KIND: 'web' | 'desktop' = isTauri() ? 'desktop' : 'web';

/** The desktop app's local server, from `backend_status` (src-tauri/src/backend.rs). */
export interface BackendStatus {
  state: 'starting' | 'ready' | 'failed';
  url: string | null;
  /** Sent as `x-api-key` with every request: the server answers nobody else. */
  key: string | null;
  error: string | null;
}

/** The desktop app's server status; null in a browser (the API is the page's own origin). */
export async function backendStatus(): Promise<BackendStatus | null> {
  if (!isTauri()) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<BackendStatus>('backend_status');
}

/** Waits for the desktop app's server (it takes a few seconds to start the first time). */
export async function waitForBackend(intervalMs = 300): Promise<BackendStatus | null> {
  for (;;) {
    const status = await backendStatus();
    if (!status || status.state !== 'starting') return status;
    await new Promise((resolve) => setTimeout(resolve, intervalMs));
  }
}

/** Stops the local server and restarts the desktop app (after the server failed to start). */
export async function restartApp(): Promise<void> {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('restart_app');
}

/** Whether this desktop build updates itself (its release feed is set), and its version (`updates.rs`). */
export interface UpdateSupport {
  enabled: boolean;
  current_version: string;
}

/** A newer version on the release feed. */
export interface AvailableUpdate {
  version: string;
  current_version: string;
  /** Unix seconds, when the feed gives a date. */
  published_at: number | null;
  notes: string | null;
}

/** Progress reported while an update downloads and installs. */
export type InstallProgress = { event: 'downloading'; downloaded: number; total: number | null } | { event: 'installing' };

/** The desktop app's update support; null in a browser (a web deployment is always current). */
export async function updateSupport(): Promise<UpdateSupport | null> {
  if (!isTauri()) return null;
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<UpdateSupport>('update_support');
}

/** Asks the release feed for a newer version: null when this one is the latest. Rejects with a message to show. */
export async function checkForUpdate(): Promise<AvailableUpdate | null> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<AvailableUpdate | null>('update_check');
}

/** Downloads, verifies and installs the update found by the last check, then restarts the app on it. */
export async function installUpdate(onProgress: (progress: InstallProgress) => void): Promise<void> {
  const { invoke, Channel } = await import('@tauri-apps/api/core');
  const channel = new Channel<InstallProgress>();
  channel.onmessage = onProgress;
  await invoke('update_install', { onProgress: channel });
}

/** Opens a URL in the system browser on desktop, or a new tab on the web. */
export async function openExternal(url: string): Promise<void> {
  if (isTauri()) {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  } else {
    window.open(url, '_blank', 'noopener,noreferrer');
  }
}

/** Saves a downloaded file: a browser download (desktop webviews handle it the same way). */
export function saveBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
