import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { DEMO_NAME } from './demoData';
import type * as DemoData from './demoData';
import { WelcomePage } from './WelcomePage';

const { build } = vi.hoisted(() => ({ build: vi.fn() }));
vi.mock('./demoData', async (importOriginal) => ({
  ...(await importOriginal<typeof DemoData>()),
  createDemoBusiness: (...args: Parameters<typeof DemoData.createDemoBusiness>) => build(...args) as Promise<string>,
}));

afterEach(() => {
  vi.restoreAllMocks();
  build.mockReset();
  window.localStorage.clear();
});

type Summary = { id: string; name: string; is_demo: boolean };

function serve(businesses: Summary[]) {
  const requests: { method: string; url: string; body: string }[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    requests.push({ method: request.method, url: request.url, body: await request.clone().text() });
    if (request.method === 'DELETE') return new Response(null, { status: 204 });
    return Response.json(businesses);
  });
  return requests;
}

describe('the welcome screen', () => {
  it('offers the demo and setting up, and remembers choosing to set up', async () => {
    serve([]);
    renderWithProviders(<WelcomePage />, { path: '/welcome' });
    expect(await screen.findByRole('heading', { name: 'Explore with demo data' })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Set up my business' }));
    expect(window.localStorage.getItem('first-run-choice')).toBe('setup');
  });

  it('builds the demo and opens it', async () => {
    serve([]);
    build.mockImplementation(async (_send: unknown, progress: (f: number) => void) => {
      progress(0.5);
      return 'demo-1';
    });
    renderWithProviders(<WelcomePage />, { path: '/welcome' });
    await userEvent.click(await screen.findByRole('button', { name: 'Explore the demo' }));
    await waitFor(() => expect(window.localStorage.getItem('selected-business')).toBe('demo-1'));
    expect(window.localStorage.getItem('first-run-choice')).toBe('demo');
  });

  it('says when the demo couldn’t be built, and offers to try again', async () => {
    serve([]);
    build.mockRejectedValue(new Error('offline'));
    renderWithProviders(<WelcomePage />, { path: '/welcome' });
    await userEvent.click(await screen.findByRole('button', { name: 'Explore the demo' }));
    expect(await screen.findByText('The demo couldn’t be set up')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeInTheDocument();
  });

  it('opens the demo there is instead of making another', async () => {
    serve([{ id: 'demo-1', name: DEMO_NAME, is_demo: true }]);
    renderWithProviders(<WelcomePage />, { path: '/welcome' });
    await userEvent.click(await screen.findByRole('button', { name: 'Open the demo' }));
    expect(window.localStorage.getItem('selected-business')).toBe('demo-1');
    expect(build).not.toHaveBeenCalled();
  });

  it('resets the demo: deletes it, keeps the owner’s own business and builds it again', async () => {
    const requests = serve([
      { id: 'mine', name: 'My Work', is_demo: false },
      { id: 'demo-1', name: DEMO_NAME, is_demo: true },
    ]);
    build.mockResolvedValue('demo-2');
    renderWithProviders(<WelcomePage />, { path: '/welcome?demo=reset' });
    await userEvent.click(await screen.findByRole('button', { name: 'Reset the demo' }));
    await waitFor(() => expect(window.localStorage.getItem('selected-business')).toBe('demo-2'));
    const deletes = requests.filter((r) => r.method === 'DELETE');
    expect(deletes).toHaveLength(1);
    expect(deletes[0]?.url).toMatch(/\/businesses\/demo-1$/);
    expect(JSON.parse(deletes[0]?.body ?? '{}')).toEqual({ confirmation_name: DEMO_NAME });
  });

  it('removes the demo and opens the owner’s own business', async () => {
    const requests = serve([
      { id: 'mine', name: 'My Work', is_demo: false },
      { id: 'demo-1', name: DEMO_NAME, is_demo: true },
    ]);
    renderWithProviders(<WelcomePage />, { path: '/welcome?demo=remove' });
    await userEvent.click(await screen.findByRole('button', { name: 'Remove the demo' }));
    await waitFor(() => expect(window.localStorage.getItem('selected-business')).toBe('mine'));
    expect(requests.filter((r) => r.method === 'DELETE')).toHaveLength(1);
    expect(build).not.toHaveBeenCalled();
  });
});

describe('the demo records', () => {
  const real = vi.importActual<typeof DemoData>('./demoData');

  /** A pretend API that answers every request with enough for the next one, recording what was sent. */
  function fakeApi(today: string, fyStart: string) {
    const sent: { method: string; path: string; body: Record<string, unknown> | undefined }[] = [];
    const send = async (method: string, path: string, body?: unknown) => {
      sent.push({ method, path, body: body as Record<string, unknown> | undefined });
      if (path.endsWith('/expense-categories') || path.endsWith('/allocations/destinations')) return [];
      if (path.endsWith('/gst/returns')) return { returns: [{ due_date: '2000-01-01', end: '1999-12-31', net: 100 }] };
      return {
        id: `id-${sent.length}`,
        today,
        current_financial_year: { start: fyStart },
        total: 115,
        subtotal: 100,
        default_rate_id: 'r',
      };
    };
    return { sent, send };
  }

  it.each([
    ['2026-11-27', '2026-04-01'],
    ['2026-04-01', '2026-04-01'],
    ['2027-03-31', '2026-04-01'],
  ])('are all dated on or before today (%s), and labelled as the demo', async (today, fyStart) => {
    const { createDemoBusiness: create } = await real;
    const { sent, send } = fakeApi(today, fyStart);
    const progress: number[] = [];
    await create(send, (f) => progress.push(f));
    expect(sent[0]).toMatchObject({ method: 'POST', path: '/businesses', body: { demo: true, name: DEMO_NAME } });
    const dates = sent.flatMap((r) =>
      Object.entries(r.body ?? {})
        .filter(([k, v]) => k !== 'due_date' && /_date$|^acquired_on$|^as_at$|^year_end$/.test(k) && typeof v === 'string')
        .map(([, v]) => v as string),
    );
    expect(dates.length).toBeGreaterThan(50);
    expect(dates.filter((d) => d > today)).toEqual([]);
    const customers = sent.filter((r) => r.path.endsWith('/customers')).map((r) => r.body?.name);
    expect(customers.every((name) => typeof name === 'string' && name.endsWith('(demo)'))).toBe(true);
    expect(progress.at(-1)).toBe(1);
    expect(Math.max(...progress.slice(0, -1))).toBeLessThan(1);
  });
});
