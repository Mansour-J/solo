import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { percent, type IncomeTax } from './api';
import { IncomeTaxPage } from './IncomeTaxPage';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-24' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const data: IncomeTax = {
  basis: 'accrual',
  overdue: 992_251,
  refunds_due: 0,
  credit: 0,
  next_due: {
    kind: 'terminal',
    label: 'Terminal tax',
    due_date: '2026-02-09',
    amount: 992_251,
    settled: 0,
    outstanding: 992_251,
    status: 'overdue',
  },
  years: [
    {
      label: 'FY2026',
      start: '2025-04-01',
      end: '2026-03-31',
      ended: true,
      income: 6_000_000,
      deductible_expenses: 0,
      taxable_income: 6_000_000,
      bands: [
        { from: 0, to: 1_560_000, rate: '0.105', income: 1_560_000, tax: 163_800 },
        { from: 1_560_000, to: 5_350_000, rate: '0.175', income: 3_790_000, tax: 663_250 },
        { from: 5_350_000, to: null, rate: '0.3', income: 650_000, tax: 195_000 },
      ],
      income_tax: 1_022_050,
      withholding_tax: 600_000,
      residual_income_tax: 422_050,
      provisional_tax: 1_041_800,
      payments: [
        {
          kind: 'provisional',
          label: 'Instalment 1 of 3',
          due_date: '2025-08-28',
          amount: 347_266,
          settled: 0,
          outstanding: 347_266,
          status: 'overdue',
        },
        {
          kind: 'terminal',
          label: 'Refund',
          due_date: '2027-02-09',
          amount: -619_750,
          settled: 0,
          outstanding: 619_750,
          status: 'refund_due',
        },
      ],
    },
    {
      label: 'FY2025',
      start: '2024-04-01',
      end: '2025-03-31',
      ended: true,
      income: 10_000_000,
      deductible_expenses: 1_000_000,
      taxable_income: 9_000_000,
      bands: [],
      income_tax: 1_992_251,
      withholding_tax: 1_000_000,
      residual_income_tax: 992_251,
      provisional_tax: null,
      payments: [
        {
          kind: 'terminal',
          label: 'Terminal tax',
          due_date: '2026-02-09',
          amount: 992_251,
          settled: 0,
          outstanding: 992_251,
          status: 'overdue',
        },
      ],
    },
  ],
};

describe('income tax', () => {
  it('shows rates as percentages', () => {
    expect(percent('0.105')).toBe('10.5%');
    expect(percent('0.1282')).toBe('12.82%');
    expect(percent('0.3')).toBe('30%');
  });

  it('shows the estimate, provisional tax and payments, and records a payment', async () => {
    const posted: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      if (request.method === 'POST') {
        posted.push(await request.json());
        return Response.json({ id: 'm1', kind: 'income_tax_payment', amount: 347_266, currency: 'NZD' }, { status: 201 });
      }
      return Response.json(data);
    });
    renderWithProviders(<IncomeTaxPage />, { path: '/income-tax' });
    expect(await screen.findByText('Income tax estimate, FY2026')).toBeInTheDocument();
    expect(screen.getByText('Residual income tax')).toBeInTheDocument();
    expect(screen.getByText(/last year’s residual income tax plus 5%/)).toBeInTheDocument();
    const instalment = screen.getByText('Provisional tax, instalment 1 of 3').closest('tr')!;
    expect(within(instalment).getByText('Overdue')).toBeInTheDocument();
    expect(screen.getByText('Refund due')).toBeInTheDocument();

    await userEvent.click(within(instalment).getByRole('button', { name: 'Record payment' }));
    const dialog = await screen.findByRole('dialog', { name: 'Record an income tax payment' });
    await userEvent.click(within(dialog).getByRole('button', { name: /Record|Save/ }));
    await waitFor(() => expect(posted).toHaveLength(1));
    expect(posted[0]).toMatchObject({ kind: 'income_tax_payment', amount: 347_266, description: 'Provisional tax, instalment 1 of 3' });
  });

  it('switches between income years', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () => Response.json(data));
    renderWithProviders(<IncomeTaxPage />, { path: '/income-tax' });
    await userEvent.click(await screen.findByRole('combobox', { name: 'Income year' }));
    await userEvent.click(await screen.findByRole('option', { name: 'FY2025', hidden: true }));
    expect(await screen.findByText('Income tax estimate, FY2025')).toBeInTheDocument();
    expect(screen.getByText(/No provisional tax for FY2025/)).toBeInTheDocument();
  });
});
