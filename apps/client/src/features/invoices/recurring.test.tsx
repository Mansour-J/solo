import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Invoice } from './api';
import { nextMonth } from './labels';
import { RecurringRunner, RepeatInvoiceModal } from './Recurring';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-25' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const invoice = { id: 'i1', issue_date: '2026-10-31', customer: { id: 'c1', name: 'Agency (test)' } } as unknown as Invoice;

describe('recurring invoices', () => {
  it('starts a month after the invoice, keeping to the month’s last day', () => {
    expect(nextMonth('2026-10-31')).toBe('2026-11-30');
    expect(nextMonth('2026-01-15')).toBe('2026-02-15');
    expect(nextMonth('2026-12-01')).toBe('2027-01-01');
  });

  it('repeats an invoice on the schedule chosen', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      writes.push(await (input as Request).json());
      return Response.json({}, { status: 201 });
    });
    renderWithProviders(<RepeatInvoiceModal invoice={invoice} opened onClose={() => undefined} />);
    await userEvent.click(screen.getByRole('checkbox', { name: /Issue each invoice straight away/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Repeat invoice' }));
    await waitFor(() =>
      expect(writes).toEqual([
        { invoice_id: 'i1', name: 'Agency (test)', frequency: 'monthly', next_date: '2026-11-30', end_date: null, auto_issue: true },
      ]),
    );
  });

  it('creates due invoices when the app opens and says so', async () => {
    const fetch = vi.spyOn(globalThis, 'fetch').mockImplementation(async () => Response.json({ created: 2, issued: 0, problems: [] }));
    renderWithProviders(<RecurringRunner />);
    expect(await screen.findByText('2 invoices were created: check and issue the drafts.')).toBeInTheDocument();
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});
