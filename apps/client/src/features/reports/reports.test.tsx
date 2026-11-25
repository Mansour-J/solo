import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import type * as Platform from '@/services/platform';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Report } from './api';
import { REPORTS, reportInfo } from './catalogue';
import { ReportsPage } from './ReportsPage';
import { ReportView } from './ReportView';

const { business, saved } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    name: 'Kererū Studio',
    currency: 'NZD',
    today: '2026-11-21',
    reporting_basis: 'accrual',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
  saved: [] as string[],
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));
vi.mock('@/services/platform', async (importOriginal) => ({
  ...(await importOriginal<typeof Platform>()),
  saveBlob: (_blob: Blob, name: string) => saved.push(name),
}));

afterEach(() => {
  vi.restoreAllMocks();
  saved.length = 0;
});

const report: Report = {
  kind: 'profit_and_loss',
  title: 'Profit and loss',
  business_name: 'Kererū Studio',
  currency: 'NZD',
  period: { from: '2026-04-01', to: '2027-03-31' },
  basis: 'accrual',
  subtitle: ['Compared with 1 Apr 2025 to 31 Mar 2026'],
  sections: [
    {
      title: 'Income',
      columns: [
        { label: 'Account', kind: 'text' },
        { label: 'Issued', kind: 'date' },
        { label: 'Invoices', kind: 'count' },
        { label: 'This period', kind: 'money' },
      ],
      rows: [['Sales', '2026-06-01', 1200, 360_000]],
      totals: ['Total income', null, 1200, 365_000],
    },
    { title: 'Empty', columns: [{ label: 'Account', kind: 'text' }], rows: [] },
  ],
  notes: ['Gross profit is revenue from sales.'],
  generated_on: '2026-11-21',
};

describe('report view', () => {
  it('formats cells by column type and shows totals, context and notes', () => {
    renderWithProviders(<ReportView report={report} />);
    expect(screen.getByText('Compared with 1 Apr 2025 to 31 Mar 2026 · Accrual basis')).toBeInTheDocument();
    expect(screen.getByRole('cell', { name: '$3,600.00' })).toBeInTheDocument();
    expect(screen.getByRole('cell', { name: '1,200' })).toBeInTheDocument();
    expect(screen.getByRole('cell', { name: '01/06/2026' })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: '$3,650.00' })).toBeInTheDocument();
    expect(screen.getByText('Nothing in this period.')).toBeInTheDocument();
    expect(screen.getByText('Gross profit is revenue from sales.')).toBeInTheDocument();
  });
});

describe('catalogue', () => {
  it('lists every report the API builds, and defaults to profit and loss', () => {
    expect(REPORTS.map((r) => r.kind)).toHaveLength(12);
    expect(REPORTS.map((r) => r.kind)).toEqual(expect.arrayContaining(['balance_sheet', 'equity_changes', 'depreciation_schedule']));
    expect(reportInfo('nonsense').kind).toBe('profit_and_loss');
    expect(reportInfo('statement_of_account').customer).toBe('required');
  });
});

function mockApi() {
  const requested: string[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const url = new URL((input as Request).url);
    requested.push(`${url.pathname}${url.search}`);
    if (url.pathname.endsWith('/export')) return new Response('pdf', { headers: { 'content-type': 'application/pdf' } });
    if (url.pathname.includes('/reports/')) return Response.json(report);
    return Response.json({ items: [], page: 1, per_page: 50, total: 0 });
  });
  return requested;
}

describe('reports page', () => {
  it('asks for a customer before building a statement', async () => {
    const requested = mockApi();
    renderWithProviders(<ReportsPage />, { path: '/reports?report=statement_of_account' });
    expect(await screen.findByText('Choose a customer')).toBeInTheDocument();
    expect(requested.some((r) => r.includes('/reports/statement_of_account'))).toBe(false);
  });

  it('shows the report and exports it with the same filters', async () => {
    const requested = mockApi();
    renderWithProviders(<ReportsPage />, { path: '/reports?report=profit_and_loss' });
    expect(await screen.findByRole('cell', { name: '$3,600.00' })).toBeInTheDocument();
    expect(requested.find((r) => r.includes('/reports/profit_and_loss'))).toContain('from=2026-04-01&to=2027-03-31');

    await userEvent.click(screen.getByRole('radio', { name: 'Cash' }));
    await waitFor(() => expect(requested.some((r) => r.includes('basis=cash'))).toBe(true));
    await userEvent.click(screen.getByRole('button', { name: 'Export' }));
    await userEvent.click(await screen.findByRole('menuitem', { name: 'PDF' }));
    await waitFor(() => expect(saved).toEqual(['Profit and loss 2026-04-01 to 2027-03-31.pdf']));
    const exported = requested.find((r) => r.includes('/export'));
    expect(exported).toContain('format=pdf');
    expect(exported).toContain('basis=cash');
  });

  it('exports every report at once, even while a statement waits for a customer', async () => {
    const requested = mockApi();
    renderWithProviders(<ReportsPage />, { path: '/reports?report=statement_of_account' });
    expect(await screen.findByText('Choose a customer')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Export' }));
    const all = await screen.findByRole('menuitem', { name: 'All reports as Excel (.xlsx)' });
    // This report's own exports wait for a customer; the export of every report doesn't.
    expect(screen.getByRole('menuitem', { name: 'PDF', hidden: true })).toBeDisabled();
    await userEvent.click(all);
    await waitFor(() => expect(saved).toEqual(['Kererū Studio - All reports 2026-04-01 to 2027-03-31.xlsx']));
    const exported = requested.find((r) => r.includes('/reports/all/export'));
    expect(exported).toContain('format=xlsx');
    expect(exported).toContain('from=2026-04-01&to=2027-03-31');
    expect(exported).not.toContain('customer_id');
  });
});
