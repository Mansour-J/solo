// The API client. There is no sign-in: the API acts for the app's one owner.
//  * desktop: the app's own local server, at the address and with the key `connectBackend` was given;
//  * web (development and tests): the page's own origin, which Vite proxies to an API.
import { createApiClient } from '@solo/api-client';
import { CLIENT_KIND, isTauri } from '@/services/platform';
import { ApiError } from './errors';

// Until `connectBackend`, the desktop app uses the compiled-in address (development).
let baseUrl = isTauri() ? (import.meta.env.VITE_API_BASE_URL || 'http://localhost:8000').replace(/\/$/, '') : window.location.origin;
let apiKey: string | null = null;

/** Points the desktop app at its local server. */
export function connectBackend(url: string, key: string | null) {
  baseUrl = url.replace(/\/$/, '');
  apiKey = key;
  client = null;
}

function send(request: Request): Promise<Response> {
  const headers = new Headers(request.headers);
  headers.set('x-client-kind', CLIENT_KIND);
  if (apiKey) headers.set('x-api-key', apiKey);
  return fetch(new Request(request, { headers, credentials: 'omit' }));
}

type ApiClient = ReturnType<typeof createApiClient>;
let client: ApiClient | null = null;

/** The typed API client, created for the current server on first use (after `connectBackend`). */
export const api: ApiClient = new Proxy({} as ApiClient, {
  get(_target, property) {
    client ??= createApiClient({ baseUrl, fetch: send });
    return Reflect.get(client, property) as unknown;
  },
});

/** Fetches a binary resource (logos, PDFs, exports). */
export async function fetchBlob(path: string): Promise<Blob> {
  const response = await send(new Request(`${baseUrl}${path}`));
  if (!response.ok) throw ApiError.fromResponse(response.status, await response.json().catch(() => null));
  return response.blob();
}

/** Uploads a file as multipart form data. */
export async function uploadFile(path: string, file: File, method: 'PUT' | 'POST' = 'PUT'): Promise<unknown> {
  const form = new FormData();
  form.append('file', file);
  const response = await send(new Request(`${baseUrl}${path}`, { method, body: form }));
  const body = await response.json().catch(() => null);
  if (!response.ok) throw ApiError.fromResponse(response.status, body);
  return body;
}

interface FetchResult<T> {
  data?: T;
  error?: unknown;
  response: Response;
}

/** Turns an openapi-fetch result into data, or throws an `ApiError`. */
export async function unwrap<T>(call: Promise<FetchResult<T>>): Promise<T> {
  let result: FetchResult<T>;
  try {
    result = await call;
  } catch {
    throw new ApiError('NETWORK', 'Unable to reach the server. Check your connection and try again.');
  }
  if (!result.response.ok) throw ApiError.fromResponse(result.response.status, result.error);
  return result.data as T;
}
