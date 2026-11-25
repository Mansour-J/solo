import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '@/test/render';
import { ReturnFigures } from './ReturnFigures';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-06-10' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

describe('GST return figures', () => {
  it('lists boxes 5 to 15 from the API for the period', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = new URL((input as Request).url);
      expect(url.pathname).toBe('/api/v1/businesses/b1/gst/returns/boxes');
      expect(url.searchParams.get('end')).toBe('2026-05-31');
      return Response.json({
        start: '2026-04-01',
        end: '2026-05-31',
        due_date: '2026-06-29',
        gst_basis: 'payments',
        box5_sales: 146_500,
        box6_zero_rated: 20_000,
        box7: 126_500,
        box8_gst: 16_500,
        box9_debit_adjustments: 0,
        box10: 16_500,
        box11_purchases: 27_025,
        box12_gst: 3_525,
        box13_credit_adjustments: 0,
        box14: 3_525,
        box15: 12_975,
      });
    });
    renderWithProviders(<ReturnFigures end="2026-05-31" onClose={() => undefined} />);
    expect(await screen.findByText('$1,465.00')).toBeInTheDocument();
    expect(screen.getByText('Box 11')).toBeInTheDocument();
    expect(screen.getByText('GST to pay')).toBeInTheDocument();
    expect(screen.getByText('$129.75')).toBeInTheDocument();
  });
});
