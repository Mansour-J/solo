import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { IncomeFormModal } from './IncomeFormModal';
import { fromIncome, gstPreview, toIncomeInput, type IncomeFormValues, type IncomeRow } from './incomeForm';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    role: 'owner',
    today: '2026-11-20',
    gst_registered: true,
    gst_registered_from: '2025-04-01',
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => vi.restoreAllMocks());

/** The customer picker loads customers; answer with an empty page and record every request. */
function mockApi() {
  return vi
    .spyOn(globalThis, 'fetch')
    .mockImplementation(async () => Response.json({ items: [], page: 1, per_page: 50, total: 0 }, { status: 200 }));
}

const values: IncomeFormValues = {
  income_date: '2026-11-01',
  payer: ' Cash sales ',
  income_type: 'sales',
  customer_id: null,
  gross_amount: 11500,
  gst_treatment: 'standard',
  gst_amount: null,
  payment_method: 'cash',
  reference: '',
  description: '',
  notes: '',
};

describe('IncomeFormModal', () => {
  it('validates before saving and previews GST at 3/23', async () => {
    const fetchSpy = mockApi();
    renderWithProviders(<IncomeFormModal opened onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole('button', { name: 'Record income' }));
    expect(await screen.findByText('Enter who paid you.')).toBeInTheDocument();
    expect(screen.getByText('Enter the amount received.')).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText(/^Amount received/), '115');
    expect(await screen.findByText('Leave empty to calculate it ($15.00).')).toBeInTheDocument();
    expect(fetchSpy.mock.calls.every(([request]) => (request as Request).method === 'GET')).toBe(true);
  });
});

describe('income form conversion', () => {
  it('rounds the GST preview like the API', () => {
    expect(gstPreview(11500)).toBe(1500);
    expect(gstPreview(100)).toBe(13);
    expect(gstPreview(0)).toBe(0);
  });

  it('sends no GST when it doesn’t apply on the date', () => {
    expect(toIncomeInput(values, business)).toMatchObject({ payer: 'Cash sales', gst_treatment: 'standard', gst_amount: null });
    expect(toIncomeInput({ ...values, income_date: '2025-03-31', gst_amount: 100 }, business)).toMatchObject({
      gst_treatment: 'no_gst',
      gst_amount: null,
    });
    expect(toIncomeInput(values, { gst_registered: false, gst_registered_from: null })).toMatchObject({ gst_treatment: 'no_gst' });
  });

  it('keeps a manual GST amount as an override when editing', () => {
    const row = {
      gross_amount: 11500,
      gst_amount: 1500,
      gst_treatment: 'standard',
      income_type: 'sales',
      payment_method: 'cash',
    } as IncomeRow;
    expect(fromIncome(row).gst_amount).toBeNull();
    expect(fromIncome({ ...row, gst_amount: 1400 }).gst_amount).toBe(1400);
  });
});
