import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { YearEndPage } from './YearEndPage';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    today: '2026-11-21',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const saved = {
  id: 'h1',
  year_end: '2026-03-31',
  business_area: '20.00',
  total_area: '200.00',
  premises_costs: 1_000_000,
  rate: 5_730,
  claim: 214_600,
  notes: '',
  updated_at: '2026-11-21T00:00:00Z',
};

describe('home office', () => {
  it('claims last year by IRD’s square metre rate and shows the server’s figure', async () => {
    const writes: unknown[] = [];
    let claim: typeof saved | null = null;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      const url = new URL(request.url);
      if (request.method === 'PUT') {
        writes.push(await request.json());
        claim = saved;
      }
      expect(url.pathname).toBe('/api/v1/businesses/b1/home-office');
      return Response.json({ year_end: '2026-03-31', rate: 5_730, claim });
    });
    renderWithProviders(<YearEndPage />, { path: '/year-end?tab=home-office' });
    expect(await screen.findByText(/a square metre/)).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText(/Business area/), '20');
    await userEvent.type(screen.getByLabelText(/Whole home/), '200');
    await userEvent.type(screen.getByLabelText(/Mortgage interest, rates and rent/), '10000');
    await userEvent.click(screen.getByRole('button', { name: 'Claim home office' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ year_end: '2026-03-31', business_area: '20', total_area: '200', premises_costs: 1_000_000 });
    expect(await screen.findByText('Claimed for the year:')).toBeInTheDocument();
    expect(screen.getByText('$2,146.00')).toBeInTheDocument();
  });

  it('says when IRD hasn’t published the rate', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => Response.json({ year_end: '2026-03-31', rate: null, claim: null }));
    renderWithProviders(<YearEndPage />, { path: '/year-end?tab=home-office' });
    expect(await screen.findByText(/hasn’t published the square metre rate/)).toBeInTheDocument();
  });
});
