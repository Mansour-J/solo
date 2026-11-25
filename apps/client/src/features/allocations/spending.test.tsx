import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { BackfillButton, SpendingPanel } from './SpendingPanel';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    today: '2026-11-25',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const balances = {
  currency: 'NZD',
  total: 20_000,
  pending_count: 0,
  has_rules: true,
  destinations: [
    {
      id: 'd1',
      name: 'Income tax',
      kind: 'income_tax',
      archived: false,
      balance: 20_000,
      target_amount: null,
      target_progress: null,
      minimum_balance: null,
      below_minimum: false,
    },
    {
      id: 'd2',
      name: 'Savings',
      kind: 'savings',
      archived: false,
      balance: 0,
      target_amount: null,
      target_progress: null,
      minimum_balance: null,
      below_minimum: false,
    },
  ],
};

describe('spending from allocations', () => {
  it('records money used from a destination that holds some', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      const path = new URL(request.url).pathname;
      if (path.endsWith('/balances')) return Response.json(balances);
      if (request.method === 'POST') {
        writes.push(await request.json());
        return Response.json({}, { status: 201 });
      }
      return Response.json([]);
    });
    renderWithProviders(<SpendingPanel />, { path: '/allocations?tab=spending' });
    await userEvent.click(await screen.findByRole('button', { name: 'Record spending' }));
    await userEvent.click(screen.getByRole('combobox', { name: /Spent from/ }));
    // Only destinations holding money are offered.
    expect(screen.queryByRole('option', { name: 'Savings', hidden: true })).not.toBeInTheDocument();
    await userEvent.click(await screen.findByRole('option', { name: 'Income tax', hidden: true }));
    await userEvent.type(screen.getByLabelText(/^Amount/), '150');
    await userEvent.type(screen.getByLabelText(/What for/), 'Provisional tax');
    await userEvent.click(screen.getAllByRole('button', { name: 'Record spending' }).at(-1)!);
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ destination_id: 'd1', spent_on: '2026-11-25', amount: 15_000, description: 'Provisional tax' });
  });

  it('opens allocations for earlier payments in a period', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      writes.push(await request.json());
      return Response.json({ opened: 3 });
    });
    renderWithProviders(<BackfillButton />, { path: '/allocations' });
    await userEvent.click(screen.getByRole('button', { name: 'Allocate earlier payments' }));
    await userEvent.click(screen.getByRole('button', { name: 'Find payments' }));
    await waitFor(() => expect(writes).toEqual([{ from: '2026-04-01', to: '2026-11-25' }]));
    expect(await screen.findByText('3 payments to review')).toBeInTheDocument();
  });
});
