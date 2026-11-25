import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Insights } from './api';
import { DashboardNotices } from './DashboardNotices';
import { formatFigure } from './format';
import { InsightsPage } from './InsightsPage';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', role: 'owner', today: '2026-11-21' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const insights: Insights = {
  generated_on: '2026-11-21',
  currency: 'NZD',
  basis: 'accrual',
  dismissed_count: 1,
  notices: [
    { key: 'overdue:1:200000', message: 'You have 1 overdue invoice ($2,000.00).', tone: 'warning', link: '/reports?report=receivables' },
  ],
  sections: [
    {
      key: 'performance',
      title: 'Financial performance',
      description: 'How revenue, expenses and profit are moving.',
      items: [],
      empty_reason: 'These appear once there are at least two complete months of income and expenses to compare.',
    },
    {
      key: 'cash_flow',
      title: 'Cash flow',
      description: 'What’s been received, what’s owed and what’s set aside.',
      empty_reason: null,
      items: [
        {
          key: 'expected-payments',
          title: 'Expected in the next 30 days',
          figure: { kind: 'money', amount: 150_000 },
          statement: 'If customers pay on time, $1,500.00 could arrive in the next 30 days from 1 invoice.',
          explanation: 'An estimate: unpaid balances of invoices due in the next 30 days.',
          tone: 'neutral',
          estimate: true,
          link: null,
          details: [{ label: 'Kākā Studio', figure: { kind: 'money', amount: 150_000 } }],
        },
      ],
    },
  ],
};

function mockApi(body: Insights = insights) {
  const sent: { url: string; body: unknown }[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    const url = new URL(request.url).pathname;
    if (request.method === 'GET') return Response.json(body);
    const text = await request.clone().text();
    sent.push({ url, body: text ? (JSON.parse(text) as unknown) : null });
    return new Response(null, { status: 204 });
  });
  return sent;
}

describe('figures', () => {
  it('formats each kind', () => {
    expect(formatFigure({ kind: 'money', amount: 116_667 }, 'NZD')).toBe('$1,166.67');
    expect(formatFigure({ kind: 'percent', amount: -25 }, 'NZD')).toBe('-25%');
    expect(formatFigure({ kind: 'days', amount: 1 }, 'NZD')).toBe('1 day');
    expect(formatFigure({ kind: 'days', amount: 20 }, 'NZD')).toBe('20 days');
  });
});

describe('insights page', () => {
  it('shows insights with their figures, marks estimates and explains empty sections', async () => {
    mockApi();
    renderWithProviders(<InsightsPage />, { path: '/insights' });
    expect(await screen.findByText('Expected in the next 30 days')).toBeInTheDocument();
    expect(screen.getByText('$1,500.00')).toBeInTheDocument();
    expect(screen.getByText('Estimate')).toBeInTheDocument();
    expect(screen.getByText(/at least two complete months/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Show the figures' }));
    expect(screen.getByText('Kākā Studio')).toBeInTheDocument();
  });

  it('dismisses a notice and can show dismissed ones again', async () => {
    const sent = mockApi();
    renderWithProviders(<InsightsPage />, { path: '/insights' });
    await userEvent.click(await screen.findByRole('button', { name: 'Dismiss: You have 1 overdue invoice ($2,000.00).' }));
    await waitFor(() => expect(sent).toContainEqual({ url: '/api/v1/businesses/b1/insights/dismiss', body: { key: 'overdue:1:200000' } }));
    await userEvent.click(screen.getByRole('button', { name: 'Show 1 dismissed notice again' }));
    await waitFor(() => expect(sent.some((s) => s.url.endsWith('/insights/restore'))).toBe(true));
  });
});

describe('dashboard notices', () => {
  it('shows nothing when there are no notices', async () => {
    mockApi({ ...insights, notices: [] });
    renderWithProviders(<DashboardNotices />);
    await waitFor(() => expect(globalThis.fetch).toHaveBeenCalled());
    // Give the query time to settle, then check the card never appeared.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByText('Worth knowing')).not.toBeInTheDocument();
  });
});
