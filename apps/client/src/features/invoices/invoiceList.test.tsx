import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { InvoicesPage } from './DocumentLists';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-11-20' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const invoice = {
  id: 'i1',
  number: 'INV-0001',
  customer_id: 'c1',
  customer_name: 'Tūī Media',
  status: 'issued',
  display_status: 'paid',
  issue_date: '2026-10-31',
  due_date: '2026-11-05',
  reference: 'PO-1001',
  currency: 'NZD',
  total: 2_511_600,
  paid: 2_511_600,
  balance: 0,
  sent_at: null,
  created_at: '2026-11-01T00:00:00Z',
};

/** The `sort` of every invoice list request, in order. */
function stubInvoices(): string[] {
  const sorts: string[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const url = new URL(input instanceof Request ? input.url : String(input));
    sorts.push(url.searchParams.get('sort') ?? '');
    return Response.json({ items: [invoice], page: 1, per_page: 25, total: 1 });
  });
  return sorts;
}

describe('invoice list', () => {
  it('sorts by any column, newest first by default, and reverses on a second click', async () => {
    const sorts = stubInvoices();
    renderWithProviders(<InvoicesPage />, { path: '/invoices' });
    expect(await screen.findByText('Tūī Media')).toBeInTheDocument();
    expect(sorts.at(-1)).toBe('-issue_date');
    expect(screen.getByRole('columnheader', { name: /Issued/ })).toHaveAttribute('aria-sort', 'descending');

    await userEvent.click(screen.getByRole('button', { name: 'Sort by customer' }));
    await waitFor(() => expect(sorts.at(-1)).toBe('customer'));
    expect(screen.getByRole('columnheader', { name: /Customer/ })).toHaveAttribute('aria-sort', 'ascending');
    expect(screen.getByRole('columnheader', { name: /Issued/ })).toHaveAttribute('aria-sort', 'none');

    await userEvent.click(screen.getByRole('button', { name: 'Sort by customer' }));
    await waitFor(() => expect(sorts.at(-1)).toBe('-customer'));

    for (const [name, sort] of [
      ['number', 'number'],
      ['due', 'due_date'],
      ['status', 'status'],
      ['total', 'total'],
      ['balance', 'balance'],
    ]) {
      await userEvent.click(screen.getByRole('button', { name: `Sort by ${name}` }));
      await waitFor(() => expect(sorts.at(-1)).toBe(sort));
    }
  });
});
