import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { api, connectBackend, unwrap } from '@/services/api/client';
import { waitForBackend, type BackendStatus } from '@/services/platform';

export type ServerInfo = components['schemas']['AppMeta'];

export const serverKeys = { info: ['server', 'info'] as const };

/** What the server says about itself: its brand, version and whether it can send email. */
export function useServerInfo() {
  return useQuery({
    queryKey: serverKeys.info,
    queryFn: () => unwrap(api.GET('/api/v1/meta')),
    staleTime: Infinity,
  });
}

/**
 * Whether the server can send email (invoices and quotes). Unknown counts as yes, so a slow answer
 * never hides features a server has; the desktop app's local server says no.
 */
export function useEmailEnabled(): boolean {
  return useServerInfo().data?.email_enabled ?? true;
}

/** The desktop app's server could not be reached: its status explains why. */
export class BackendUnavailable extends Error {
  constructor(readonly status: BackendStatus) {
    super(status.error ?? 'The server isn’t available.');
  }
}

/**
 * At start-up on desktop: waits for the app's own server and points the API client at it. Does nothing
 * in a browser. Throws `BackendUnavailable` when the server couldn't start.
 */
export async function prepareBackend(): Promise<BackendStatus | null> {
  const status = await waitForBackend();
  if (!status) return null;
  if (status.state !== 'ready' || !status.url) throw new BackendUnavailable(status);
  connectBackend(status.url, status.key);
  return status;
}
