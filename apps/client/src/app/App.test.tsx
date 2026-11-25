import type * as Platform from '@/services/platform';
import { render, screen, waitFor } from '@testing-library/react';
import { App } from './App';

const SERVER = 'http://127.0.0.1:5555';
const KEY = 'per-start-key';

vi.mock('@/services/platform', async (importOriginal) => ({
  ...(await importOriginal<typeof Platform>()),
  // The desktop app, whose local server is ready a moment after the page loads.
  isTauri: () => true,
  waitForBackend: () => new Promise((resolve) => setTimeout(() => resolve({ state: 'ready', url: SERVER, key: KEY, error: null }), 30)),
}));

const PROFILE = {
  id: 'u1',
  first_name: '',
  last_name: '',
  created_at: '2026-11-23T00:00:00Z',
  preferences: {
    theme: 'light',
    language: 'en-NZ',
    date_format: 'DD/MM/YYYY',
    number_locale: 'en-NZ',
    notifications: { product_updates: false, overdue_invoice_reminders: true },
  },
};

describe('starting the desktop app', () => {
  it('opens straight into the books, asking only the app’s own server with its key', async () => {
    const requests: Request[] = [];
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo | URL) => {
        const request = input instanceof Request ? input : new Request(String(input));
        requests.push(request);
        const path = new URL(request.url).pathname;
        const body = path === '/api/v1/users/me' ? PROFILE : path === '/api/v1/businesses' ? [] : { brand: {}, email_enabled: false };
        return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } });
      }),
    );
    render(<App />);
    // No sign-in: a first start goes to setting up a business.
    expect(await screen.findByRole('heading', { name: 'Kia ora' })).toBeInTheDocument();
    await waitFor(() => expect(requests.some((r) => r.url.endsWith('/api/v1/users/me'))).toBe(true));
    expect(requests.filter((r) => !r.url.startsWith(SERVER))).toEqual([]);
    expect(requests.every((r) => r.headers.get('x-api-key') === KEY)).toBe(true);
    vi.unstubAllGlobals();
  });
});
