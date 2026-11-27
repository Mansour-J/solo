import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { BusinessHealth, HealthCheck } from './api';
import { BusinessHealthPage } from './BusinessHealthPage';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-26' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const passed = (key: string, title: string, finding: string): HealthCheck => ({
  key,
  area: 'details',
  title,
  status: 'passed',
  finding,
  why: 'Because.',
  action: null,
  link: null,
});

const health: BusinessHealth = {
  checked_at: '2026-11-26T02:00:00Z',
  checks: [
    {
      key: 'gst-number',
      area: 'gst',
      title: 'GST number',
      status: 'critical',
      finding: 'You’re registered for GST but your GST number isn’t in your business details, so it isn’t on your invoices.',
      why: 'A tax invoice must show your GST number.',
      action: 'Add your GST number',
      link: '/settings/business',
    },
    {
      key: 'receipts',
      area: 'records',
      title: 'Receipts',
      status: 'warning',
      finding: '2 expenses this financial year have no receipt, including one that claims GST over $50.',
      why: 'IRD expects you to keep records.',
      action: 'Attach receipts',
      link: '/expenses?receipt=missing',
    },
    passed('address', 'Business address', 'Your address is on your invoices.'),
    {
      ...passed('customer-emails', 'Customers who owe you', 'No customer owes you money.'),
      status: 'not_applicable',
      area: 'getting_paid',
    },
  ],
};

function serve(...bodies: BusinessHealth[]) {
  const requests: string[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    requests.push((input as Request).url);
    return Response.json(bodies[Math.min(requests.length, bodies.length) - 1]);
  });
  return requests;
}

describe('business health', () => {
  it('puts problems first, with why they matter and a way to fix them', async () => {
    const requests = serve(health);
    renderWithProviders(<BusinessHealthPage />, { path: '/settings/health' });
    expect(await screen.findByText('Needs attention')).toBeInTheDocument();
    expect(requests[0]).toContain('/api/v1/businesses/b1/health');

    expect(screen.getByText('To fix now').previousSibling).toHaveTextContent('1');
    expect(screen.getAllByText('Worth fixing')[0]?.previousSibling).toHaveTextContent('1');
    expect(screen.getByText('Passed', { selector: 'p' }).previousSibling).toHaveTextContent('1');

    expect(screen.getByText(/GST number isn’t in your business details/)).toBeInTheDocument();
    expect(screen.getByText('A tax invoice must show your GST number.')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Add your GST number' })).toHaveAttribute('href', '/settings/business');
    expect(screen.getByRole('link', { name: 'Attach receipts' })).toHaveAttribute('href', '/expenses?receipt=missing');
    expect(screen.getByText('GST', { selector: 'p' })).toBeInTheDocument();

    // Checks that passed or don't apply are folded away.
    await userEvent.click(screen.getByRole('button', { name: 'Passed (1)' }));
    // The section opens with an animation, so wait for it rather than checking straight away.
    const passed = await screen.findByText('Your address is on your invoices.');
    await waitFor(() => expect(passed).toBeVisible());
    expect(screen.getByRole('button', { name: 'Doesn’t apply (1)' })).toBeInTheDocument();
  });

  it('checks again', async () => {
    const fixed: BusinessHealth = {
      checked_at: '2026-11-26T02:05:00Z',
      checks: [passed('address', 'Business address', 'Your address is on your invoices.')],
    };
    const requests = serve(health, fixed);
    renderWithProviders(<BusinessHealthPage />, { path: '/settings/health' });
    await screen.findByText('Needs attention');
    await userEvent.click(screen.getByRole('button', { name: 'Check again' }));
    expect(await screen.findByText('All set')).toBeInTheDocument();
    expect(requests).toHaveLength(2);
    expect(screen.getByText('Everything that applies to your business is in place.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Doesn’t apply/ })).not.toBeInTheDocument();
  });

  it('says how serious the rest is', async () => {
    const only = (status: HealthCheck['status']): BusinessHealth => ({
      checked_at: '2026-11-26T02:00:00Z',
      checks: [{ ...health.checks[1]!, status }],
    });
    serve(only('warning'));
    const { unmount } = renderWithProviders(<BusinessHealthPage />, { path: '/settings/health' });
    expect(await screen.findByText('A few things to look at')).toBeInTheDocument();
    unmount();
    vi.restoreAllMocks();
    serve(only('recommendation'));
    renderWithProviders(<BusinessHealthPage />, { path: '/settings/health' });
    expect(await screen.findByText('In good shape')).toBeInTheDocument();
    expect(screen.getByText('Suggestion')).toBeInTheDocument();
  });

  it('shows an error it can retry', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      Response.json({ error: { code: 'internal', message: 'Boom' } }, { status: 500 }),
    );
    renderWithProviders(<BusinessHealthPage />, { path: '/settings/health' });
    await waitFor(() => expect(screen.getByRole('button', { name: /try again|retry/i })).toBeInTheDocument());
  });
});
