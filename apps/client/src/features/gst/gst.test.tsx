import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { periodEndMonths, periodLabel, type GstReturns } from './api';
import { GstReturnsPage } from './GstReturnsPage';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-24', gst_registered: true } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const returns: GstReturns = {
  gst_registered: true,
  gst_period: 'two_monthly',
  gst_period_end_month: 3,
  gst_basis: 'payments',
  today: '2026-11-24',
  brought_forward: null,
  returns: [
    {
      start: '2026-04-01',
      end: '2026-05-31',
      due_date: '2026-06-29',
      collected: 15_000,
      claimable: 1_500,
      adjustments: 0,
      net: 13_500,
      settled: 13_500,
      outstanding: 0,
      status: 'paid',
    },
    {
      start: '2026-06-01',
      end: '2026-07-31',
      due_date: '2026-08-28',
      collected: 6_000,
      claimable: 0,
      adjustments: 0,
      net: 6_000,
      settled: 0,
      outstanding: 6_000,
      status: 'overdue',
    },
    {
      start: '2026-10-01',
      end: '2026-11-30',
      due_date: '2027-01-15',
      collected: 3_000,
      claimable: 0,
      adjustments: 0,
      net: 3_000,
      settled: 0,
      outstanding: 3_000,
      status: 'in_progress',
    },
  ],
  owing_now: 6_000,
  overdue: 6_000,
  refunds_due: 0,
  credit: 0,
  next_due: { end: '2026-07-31', due_date: '2026-08-28', amount: 6_000, status: 'overdue' },
};

function stub(data: GstReturns) {
  const posted: unknown[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    if (request.method === 'POST') {
      posted.push(await request.json());
      return Response.json({ id: 'm1', kind: 'gst_payment', amount: 6_000, currency: 'NZD' }, { status: 201 });
    }
    return Response.json(data);
  });
  return posted;
}

describe('GST returns', () => {
  it('labels periods and the months they end in', () => {
    expect(periodLabel('2026-04-01', '2026-05-31')).toBe('Apr–May 2026');
    expect(periodLabel('2026-10-01', '2027-03-31')).toBe('Oct 2026–Mar 2027');
    expect(periodLabel('2026-05-01', '2026-05-31')).toBe('May 2026');
    expect(periodEndMonths('two_monthly', 3)).toBe('Jan, Mar, May, Jul, Sept and Nov');
    expect(periodEndMonths('six_monthly', 3)).toBe('Mar and Sept');
  });

  it('shows what to pay IRD, each period newest first, and records a payment for one', async () => {
    const posted = stub(returns);
    renderWithProviders(<GstReturnsPage />, { path: '/gst' });
    expect(await screen.findByText('To pay IRD now')).toBeInTheDocument();
    expect(screen.getByText(/of it is past its due date/)).toBeInTheDocument();
    const rows = screen.getAllByRole('row').slice(1);
    expect(within(rows[0]!).getByText('Oct–Nov 2026')).toBeInTheDocument();
    expect(within(rows[0]!).getByText('In progress')).toBeInTheDocument();
    expect(within(rows[2]!).getByText('Paid')).toBeInTheDocument();
    expect(screen.getByText(/periods ending Jan, Mar, May, Jul, Sept and Nov, on the payments basis/)).toBeInTheDocument();

    const overdue = rows[1]!;
    expect(within(overdue).getByText('Overdue')).toBeInTheDocument();
    await userEvent.click(within(overdue).getByRole('button', { name: 'Record payment' }));
    const dialog = await screen.findByRole('dialog', { name: 'Record a GST payment' });
    expect(within(dialog).getByDisplayValue('GST return Jun–Jul 2026')).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole('button', { name: /Record|Save/ }));
    await waitFor(() => expect(posted).toHaveLength(1));
    expect(posted[0]).toMatchObject({ kind: 'gst_payment', amount: 6_000, description: 'GST return Jun–Jul 2026' });
  });

  it('explains what to do when the business isn’t registered for GST', async () => {
    stub({ ...returns, gst_registered: false, returns: [], next_due: null, owing_now: 0, overdue: 0 });
    renderWithProviders(<GstReturnsPage />, { path: '/gst' });
    expect(await screen.findByText('Not registered for GST')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open business details' })).toHaveAttribute(
      'href',
      expect.stringContaining('/settings/business'),
    );
  });
});
