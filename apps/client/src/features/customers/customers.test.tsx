import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { CustomerFormModal } from './CustomerFormModal';
import { fromCustomer, isValidNzbn, toCustomerInput, type Customer } from './customerForm';

// The form reads the current business and its default payment terms; stub them so the only
// network traffic in these tests is what the form itself sends.
const { business } = vi.hoisted(() => ({
  business: { id: 'b1', currency: 'NZD', role: 'owner', today: '2026-11-20' } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
  useBusinessSettings: () => ({ data: { default_payment_terms_days: 20 } }),
}));

afterEach(() => vi.restoreAllMocks());

const address = { line1: '', line2: '', suburb: '', city: '', region: '', postcode: '', country: 'NZ' };
const customer: Customer = {
  id: 'c1',
  name: 'Kōwhai Design',
  send_reminders: true,
  business_name: '',
  contact_person: 'Mere',
  email: 'mere@example.com',
  phone: null,
  billing_address: address,
  business_address: address,
  nzbn: null,
  tax_number: null,
  notes: '',
  payment_terms_days: null,
  default_currency: 'AUD',
  is_active: true,
  outstanding: 0,
  overdue: 0,
  created_at: '2026-01-01T00:00:00Z',
  updated_at: '2026-01-01T00:00:00Z',
};

describe('CustomerFormModal', () => {
  it('validates before contacting the server', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    renderWithProviders(<CustomerFormModal opened onClose={() => undefined} />);
    await userEvent.type(await screen.findByLabelText('Email'), 'not-an-email');
    await userEvent.type(screen.getByLabelText('NZBN'), '9429041234564');
    await userEvent.click(screen.getByRole('button', { name: 'Add customer' }));
    expect(await screen.findByText('Enter the customer’s name.')).toBeInTheDocument();
    expect(screen.getByText('Enter a valid email address.')).toBeInTheDocument();
    expect(screen.getByText('Enter a valid 13-digit NZBN.')).toBeInTheDocument();
    expect(screen.getByText('Leave empty to use your default (20 days).')).toBeInTheDocument();
    expect(fetchSpy).not.toHaveBeenCalled();
  });

  it('puts server errors on nested address fields', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      Response.json(
        {
          error: {
            code: 'VALIDATION',
            message: 'Some fields need attention.',
            fields: { 'billing_address.city': 'Use 120 characters or fewer.' },
          },
        },
        { status: 422 },
      ),
    );
    renderWithProviders(<CustomerFormModal opened onClose={() => undefined} />);
    await userEvent.type(await screen.findByLabelText(/^Name/), 'Aroha Ngata');
    await userEvent.click(screen.getByRole('button', { name: 'Add customer' }));
    await waitFor(() =>
      expect(screen.getByLabelText('Billing address: town or city')).toHaveAccessibleDescription(/Use 120 characters or fewer\./),
    );
  });
});

describe('customer form conversion', () => {
  it('checks the NZBN check digit', () => {
    expect(isValidNzbn('9429041234563')).toBe(true);
    expect(isValidNzbn('9429-0412-3456-3')).toBe(true);
    expect(isValidNzbn('9429041234564')).toBe(false);
    expect(isValidNzbn('942904123456')).toBe(false);
  });

  it('sends empty optional fields as null and keeps fields the form doesn’t edit', () => {
    const input = toCustomerInput({ ...fromCustomer(customer), payment_terms_days: '' }, customer);
    expect(input).toMatchObject({ phone: null, nzbn: null, payment_terms_days: null, default_currency: 'AUD', is_active: true });
    expect(toCustomerInput({ ...fromCustomer(customer), payment_terms_days: 7 }, customer).payment_terms_days).toBe(7);
  });
});
