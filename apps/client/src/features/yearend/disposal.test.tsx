import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Asset } from './api';
import { AssetDisposal } from './AssetDisposal';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-08-10' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const laptop = { id: 'a1', name: 'Laptop', disposed_on: null } as unknown as Asset;

describe('selling or scrapping an asset', () => {
  it('records the sale with what was received', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      expect(new URL(request.url).pathname).toBe('/api/v1/businesses/b1/assets/a1/disposal');
      writes.push(await request.json());
      return Response.json({});
    });
    renderWithProviders(<AssetDisposal asset={laptop} />);
    await userEvent.click(screen.getByRole('button', { name: 'Sell or scrap' }));
    await userEvent.type(screen.getByLabelText(/Amount received/), '2300');
    await userEvent.click(screen.getByRole('button', { name: 'Record sale' }));
    await waitFor(() => expect(writes).toEqual([{ disposed_on: '2026-08-10', sale_price: 230_000, notes: '' }]));
  });

  it('shows the API’s result for a sold asset', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      Response.json({
        id: 'd1',
        asset_id: 'a1',
        disposed_on: '2026-08-10',
        sale_price: 230_000,
        sale_gst: 30_000,
        book_value: 175_000,
        recovered: 25_000,
        loss: 0,
        capital_gain: 0,
        notes: '',
      }),
    );
    renderWithProviders(<AssetDisposal asset={{ ...laptop, disposed_on: '2026-08-10' }} />);
    expect(await screen.findByText('Depreciation recovered (taxable income)')).toBeInTheDocument();
    expect(screen.getByText('$250.00')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo sale' })).toBeInTheDocument();
  });
});
