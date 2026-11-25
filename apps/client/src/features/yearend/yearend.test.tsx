import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Asset, Movement } from './api';
import { assetBody, assetSchema, isInflow, movementLabel, openingBody, recentYearEnds, type AssetValues } from './forms';
import { YearEndPage } from './YearEndPage';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    today: '2026-11-21',
    gst_registered: true,
    reporting_basis: 'accrual',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => {
  vi.restoreAllMocks();
});

const drawing: Movement = {
  id: 'm1',
  movement_date: '2024-04-25',
  kind: 'drawing',
  amount: 1_180_000,
  currency: 'NZD',
  description: 'Transfer to personal account',
  reference: '',
  voided_at: null,
  void_reason: null,
  created_at: '2024-04-25T00:00:00Z',
};

const tablet: Asset = {
  id: 'a1',
  name: 'Tablet',
  asset_group: 'Computer equipment',
  cost: 149_900,
  purchase_gst: 0,
  acquired_on: '2023-02-20',
  method: 'dv',
  rate: '67.00',
  private_use: '0.00',
  opening_value: 98_934,
  book_value: 32_648,
  notes: '',
  archived: false,
  created_at: '2024-04-01T00:00:00Z',
};

/** Answers the year-end endpoints; records every write. */
function mockApi() {
  const writes: { method: string; path: string; body: unknown }[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    const url = new URL(request.url);
    if (request.method !== 'GET') {
      const text = await request.text();
      writes.push({ method: request.method, path: url.pathname, body: text ? JSON.parse(text) : null });
      if (url.pathname.endsWith('/cash-movements')) return Response.json(drawing, { status: 201 });
      if (url.pathname.endsWith('/opening-balances')) return Response.json({ balances: null, fixed_assets: 0, retained_earnings: null });
      if (url.pathname.endsWith('/assets/depreciation'))
        return Response.json({ year_end: '2026-03-31', lines: [], depreciation: 0, private_amount: 0 });
      return Response.json(tablet);
    }
    if (url.pathname.endsWith('/cash-movements')) return Response.json({ items: [drawing], page: 1, per_page: 25, total: 1 });
    if (url.pathname.endsWith('/assets')) return Response.json([tablet]);
    if (url.pathname.endsWith('/assets/depreciation'))
      return Response.json({
        year_end: url.searchParams.get('year_end'),
        lines: [
          {
            asset_id: 'a1',
            name: 'Tablet',
            asset_group: 'Computer equipment',
            cost: 149_900,
            id: 'd1',
            year_end: url.searchParams.get('year_end'),
            opening_value: 98_934,
            purchases: 0,
            depreciation: 66_286,
            private_amount: 0,
            closing_value: 32_648,
          },
        ],
        depreciation: 66_286,
        private_amount: 0,
      });
    if (url.pathname.endsWith('/opening-balances'))
      return Response.json({
        balances: {
          id: 'o1',
          as_at: '2024-03-31',
          bank: 4_825_000,
          gst_payable: 213_450,
          drawings: -9_640_000,
          funds_introduced: 1_250_000,
          personal_accounts: -320_000,
          withholding_tax: -5_470_000,
          income_tax: 0,
          updated_at: '2024-04-01T00:00:00Z',
        },
        fixed_assets: 433_865,
        retained_earnings: 19_225_415,
      });
    return Response.json({});
  });
  return writes;
}

describe('year-end forms', () => {
  it('labels movement kinds and knows which bring money in', () => {
    expect(movementLabel('gst_payment')).toBe('GST payment');
    expect(movementLabel('unknown')).toBe('unknown');
    expect(isInflow('funds_introduced')).toBe(true);
    expect(isInflow('income_tax_refund')).toBe(true);
    expect(isInflow('drawing')).toBe(false);
  });

  it('sends drawings and taxes as the balance sheet shows them', () => {
    expect(
      openingBody({
        as_at: '2024-03-31',
        bank: 100,
        gst_payable: null,
        drawings: 500,
        funds_introduced: 50,
        personal_accounts: -20,
        withholding_tax: 30,
        income_tax: null,
      }),
    ).toEqual({
      as_at: '2024-03-31',
      bank: 100,
      gst_payable: 0,
      drawings: -500,
      funds_introduced: 50,
      personal_accounts: -20,
      withholding_tax: -30,
      income_tax: -0,
    });
  });

  it('only sends a brought-forward value for assets brought forward, and needs one', () => {
    const values: AssetValues = {
      name: ' Camera ',
      asset_group: 'Computer equipment',
      cost: 219_500,
      purchase_gst: 28_630,
      acquired_on: '2022-12-02',
      method: 'dv',
      rate: '40',
      private_use: '',
      brought_forward: true,
      opening_value: null,
      notes: '',
    };
    const result = assetSchema.safeParse(values);
    expect(result.success).toBe(false);
    expect(result.error?.issues.map((i) => i.path.join('.'))).toContain('opening_value');
    const body = assetBody({ ...values, opening_value: 131_700 });
    expect(body).toMatchObject({ name: 'Camera', purchase_gst: 0, opening_value: 131_700, private_use: '0' });
    expect(assetBody({ ...values, brought_forward: false })).toMatchObject({ purchase_gst: 28_630, opening_value: null });
    expect(assetSchema.safeParse({ ...values, opening_value: 1, rate: '0' }).success).toBe(false);
    expect(assetSchema.safeParse({ ...values, opening_value: 1, private_use: '100' }).success).toBe(false);
  });

  it('lists recent year ends, keeping the last day of February', () => {
    expect(recentYearEnds('2027-03-31', 3)).toEqual(['2027-03-31', '2026-03-31', '2025-03-31']);
    expect(recentYearEnds('2025-02-28', 2)).toEqual(['2025-02-28', '2024-02-29']);
    expect(recentYearEnds('2025-06-15', 1)).toEqual(['2025-06-15']);
  });
});

describe('YearEndPage', () => {
  it('lists money in and out and records a drawing', async () => {
    const writes = mockApi();
    renderWithProviders(<YearEndPage />, { path: '/year-end' });
    expect(await screen.findByText('Transfer to personal account')).toBeInTheDocument();
    expect(screen.getByText(/11,800.00/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Record money in or out' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Record' }));
    expect(await screen.findByText('Enter the amount.')).toBeInTheDocument();
    expect(writes).toHaveLength(0);

    await userEvent.type(screen.getByLabelText(/^Amount/), '250');
    await userEvent.type(screen.getByLabelText(/^Description/), '  Rent  ');
    await userEvent.click(screen.getByRole('button', { name: 'Record' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      method: 'POST',
      path: '/api/v1/businesses/b1/cash-movements',
      body: { kind: 'drawing', amount: 25_000, movement_date: '2026-11-21', description: 'Rent' },
    });
  });

  it('shows assets with their book value and posts a year’s depreciation', async () => {
    const writes = mockApi();
    renderWithProviders(<YearEndPage />, { path: '/year-end?tab=assets' });
    expect(await screen.findByText('Tablet')).toBeInTheDocument();
    expect(screen.getByText('67% DV')).toBeInTheDocument();
    expect(screen.getAllByText('$326.48').length).toBeGreaterThan(0);
    expect((await screen.findAllByText('$662.86')).length).toBe(2);
    await userEvent.click(screen.getByRole('button', { name: 'Post again' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({ path: '/api/v1/businesses/b1/assets/depreciation', body: { year_end: '2026-03-31' } });
  });

  it('shows the opening balances as positive amounts with the balancing figure', async () => {
    const writes = mockApi();
    renderWithProviders(<YearEndPage />, { path: '/year-end?tab=opening' });
    expect(await screen.findByText('$192,254.15')).toBeInTheDocument();
    expect(screen.getByText('$4,338.65')).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: /^Drawings to date/ })).toHaveValue('96400.00');
    expect(screen.getByRole('textbox', { name: /^Business bank account/ })).toHaveValue('48250.00');
    await userEvent.click(screen.getByRole('button', { name: 'Save opening balances' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]?.body).toMatchObject({ as_at: '2024-03-31', drawings: -9_640_000, withholding_tax: -5_470_000, bank: 4_825_000 });
  });
});
