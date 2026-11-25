import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen } from '@testing-library/react';
import { renderWithProviders } from '@/test/render';
import { daysLeftLabel, type KeyDate } from './api';
import { KeyDates } from './KeyDates';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-25' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const date = (over: Partial<KeyDate>): KeyDate => ({
  kind: 'gst_return',
  title: 'GST return',
  detail: '1 Oct – 30 Nov 2026',
  date: '2027-01-15',
  days_left: 51,
  amount: null,
  urgency: 'upcoming',
  link: '/gst',
  ...over,
});

describe('key dates', () => {
  it('says how many days are left, in words', () => {
    expect(daysLeftLabel(0)).toBe('Today');
    expect(daysLeftLabel(1)).toBe('Tomorrow');
    expect(daysLeftLabel(12)).toBe('In 12 days');
    expect(daysLeftLabel(-1)).toBe('1 day overdue');
    expect(daysLeftLabel(-30)).toBe('30 days overdue');
  });

  it('shows each date the API gives, overdue ones marked, and links to where to deal with it', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      expect(new URL((input as Request).url).pathname).toBe('/api/v1/businesses/b1/key-dates');
      return Response.json({
        today: '2026-11-25',
        items: [
          date({ date: '2026-05-07', days_left: -202, urgency: 'overdue', amount: 15_000, detail: '1 Feb – 31 Mar 2026' }),
          date({
            kind: 'provisional_tax',
            title: 'Provisional tax',
            detail: 'Instalment 2 of 3, FY2027',
            date: '2027-01-15',
            days_left: 51,
            link: '/income-tax',
          }),
          date({
            kind: 'year_end',
            title: 'Financial year ends',
            detail: 'Then post depreciation',
            date: '2027-03-31',
            days_left: 126,
            link: '/year-end',
          }),
        ],
      });
    });
    renderWithProviders(<KeyDates />, { path: '/dashboard' });
    expect(await screen.findByText('202 days overdue')).toBeInTheDocument();
    expect(screen.getByText('Overdue')).toBeInTheDocument();
    expect(screen.getByText('$150.00')).toBeInTheDocument();
    expect(screen.getByText('In 51 days')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /Provisional tax, In 51 days/ })).toHaveAttribute('href', '/income-tax');
    expect(screen.getByRole('link', { name: /Financial year ends/ })).toHaveAttribute('href', '/year-end');
  });
});
