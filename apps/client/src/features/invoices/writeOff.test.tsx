import type { Business, components } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { PaymentForm } from './DocumentModals';

const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', today: '2026-06-15' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

const invoice = { id: 'i1', currency: 'NZD', balance: 57_500, number: 'INV-0001' } as unknown as components['schemas']['Invoice'];

describe('writing off a bad debt', () => {
  it('writes off what is left, with no tax withheld, and explains the GST', async () => {
    const posted: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      posted.push(await (input as Request).json());
      return Response.json({ ...invoice, display_status: 'written_off', balance: 0 }, { status: 201 });
    });
    renderWithProviders(<PaymentForm invoice={invoice} writeOff onDone={() => undefined} />);
    expect(screen.getByText(/as a bad debt/)).toBeInTheDocument();
    expect(screen.getByText(/comes off your GST return for the period you write it off/)).toBeInTheDocument();
    expect(screen.queryByLabelText('Tax withheld')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /Record|Save/ }));
    await waitFor(() => expect(posted).toHaveLength(1));
    expect(posted[0]).toMatchObject({ method: 'write_off', amount: 57_500, withholding_tax: 0, payment_date: '2026-06-15' });
  });
});
