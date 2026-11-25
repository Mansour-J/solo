import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Asset, AssetUse } from './api';
import { ChangeOfUse } from './ChangeOfUse';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-25' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const car = { id: 'a1', name: 'Car', cost: 2_500_000, archived: false } as Asset;
const period = (period_end: string, over: Partial<AssetUse['periods'][number]> = {}) => ({
  period_end,
  months: 12,
  business_use: null,
  notes: '',
  actual_use: null,
  previous_use: null,
  adjustment: null,
  ended: false,
  reviewable: false,
  ...over,
});
const before: AssetUse = {
  asset_id: 'a1',
  applies: true,
  reason: null,
  full_gst: 375_000,
  intended_use: '80.00',
  total_adjustment: 0,
  periods: [period('2026-03-31', { months: 11, ended: true, reviewable: true }), period('2027-03-31'), period('2028-03-31')],
};
const after: AssetUse = {
  ...before,
  total_adjustment: 75_000,
  periods: [
    period('2026-03-31', {
      months: 11,
      ended: true,
      business_use: '100.00',
      actual_use: '100.0',
      previous_use: '80.00',
      adjustment: 75_000,
    }),
    period('2027-03-31'),
    period('2028-03-31'),
  ],
};

describe('GST change of use', () => {
  it('records the business use for a period and shows the adjustment the API works out', async () => {
    const writes: unknown[] = [];
    let current = before;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      expect(new URL(request.url).pathname).toBe('/api/v1/businesses/b1/assets/a1/use-reviews');
      if (request.method === 'PUT') {
        writes.push(await request.json());
        current = after;
      }
      return Response.json(current);
    });
    renderWithProviders(<ChangeOfUse assets={[car]} />, { path: '/year-end?tab=assets' });
    expect(await screen.findByText('Car')).toBeInTheDocument();
    expect(screen.getByText('After 31/03/2027')).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText(/Business use to/), '100');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(writes).toEqual([{ period_end: '2026-03-31', business_use: '100' }]));
    expect(await screen.findByText('Claim')).toBeInTheDocument();
    expect(screen.getByText('$750.00')).toBeInTheDocument();
  });

  it('refuses a percentage outside 0 to 100 before sending it', async () => {
    const fetch = vi.spyOn(globalThis, 'fetch').mockImplementation(async () => Response.json(before));
    renderWithProviders(<ChangeOfUse assets={[car]} />, { path: '/year-end?tab=assets' });
    await userEvent.type(await screen.findByLabelText(/Business use to/), '120');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Enter a percentage between 0 and 100.')).toBeInTheDocument();
    expect(fetch.mock.calls.every(([r]) => (r as Request).method === 'GET')).toBe(true);
  });

  it('explains when an asset needs no adjustments', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      Response.json({ ...before, applies: false, reason: 'No GST was claimed on this asset, so there’s nothing to adjust.', periods: [] }),
    );
    renderWithProviders(<ChangeOfUse assets={[car]} />, { path: '/year-end?tab=assets' });
    expect(await screen.findByText(/nothing to adjust/)).toBeInTheDocument();
  });
});
