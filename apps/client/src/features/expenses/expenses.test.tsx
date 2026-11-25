import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type { Expense } from './api';
import { emptyExpenseValues, expenseSchema, fromExpense, toExpenseInput, type ExpenseFormValues } from './expenseForm';
import { ExpenseFormModal } from './ExpenseFormModal';
import { defaultDeductibleHundredths, formatShare, gstRegisteredOn, hundredthsToString, percentHundredths } from './expenseMath';

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

/** Answers the requests the form makes (settings, categories, the API's calculation); records them. */
function mockApi() {
  return vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const url = (input as Request).url;
    if (url.endsWith('/expenses/calculate')) {
      return Response.json({
        registered: true,
        gst_treatment: 'standard',
        gst_rate: '0.15',
        gst_amount: 1_500,
        business_percentage: '100.00',
        deductible_percentage: '100.00',
        split: { expense_amount: 10_000, gst_claim: 1_500, private_amount: 0 },
      });
    }
    if (url.endsWith('/settings')) return Response.json({ default_deductible_percentage: '100.00' });
    if (url.endsWith('/expense-categories')) {
      return Response.json([
        { id: 'c1', name: 'Software', account_code: '5000', archived: false, expense_count: 0 },
        { id: 'c2', name: 'Old category', account_code: '5010', archived: true, expense_count: 3 },
      ]);
    }
    return Response.json({ items: [], page: 1, per_page: 50, total: 0 });
  });
}

describe('expense form helpers', () => {
  it('parses percentages to hundredths and back', () => {
    expect(percentHundredths('62.5')).toBe(6_250);
    expect(percentHundredths(62.5)).toBe(6_250);
    expect(percentHundredths('100.00')).toBe(10_000);
    expect(percentHundredths('100.01')).toBeNull();
    expect(percentHundredths('12.345')).toBeNull();
    expect(percentHundredths('-1')).toBeNull();
    expect(percentHundredths('')).toBeNull();
    expect(hundredthsToString(6_250)).toBe('62.5');
    expect(hundredthsToString(10_000)).toBe('100');
    expect(hundredthsToString(5)).toBe('0.05');
    expect(formatShare('62.50')).toBe('62.5%');
    expect(formatShare(null)).toBe('—');
  });

  it('caps the default deductible share at the business share', () => {
    expect(defaultDeductibleHundredths('business', 10_000, '100')).toBe(10_000);
    expect(defaultDeductibleHundredths('partial', 6_000, '50')).toBe(5_000);
    expect(defaultDeductibleHundredths('partial', 4_000, '50')).toBe(4_000);
    expect(defaultDeductibleHundredths('personal', 0, '100')).toBe(0);
    expect(defaultDeductibleHundredths('partial', null, '100')).toBeNull();
  });

  it('knows when the business is registered for GST', () => {
    expect(gstRegisteredOn(business, '2025-04-01')).toBe(true);
    expect(gstRegisteredOn(business, '2025-03-31')).toBe(false);
    expect(gstRegisteredOn({ gst_registered: false, gst_registered_from: null }, '2026-01-01')).toBe(false);
  });
});

describe('expense form conversion', () => {
  const values: ExpenseFormValues = {
    ...emptyExpenseValues('2026-11-01', true),
    supplier: '  Spark  ',
    category_id: 'c1',
    gross_amount: 11_500,
  };

  it('sends trimmed values, and no GST when the business isn’t registered on the date', () => {
    expect(toExpenseInput(values, true)).toMatchObject({
      supplier: 'Spark',
      gst_treatment: 'standard',
      gst_amount: null,
      business_percentage: null,
      deductible_percentage: null,
      rechargeable_customer_id: null,
    });
    expect(toExpenseInput({ ...values, gst_override: true, gst_amount: 1_400 }, true).gst_amount).toBe(1_400);
    expect(toExpenseInput({ ...values, gst_override: true, gst_amount: 1_400 }, false)).toMatchObject({
      gst_treatment: 'no_gst',
      gst_amount: null,
    });
  });

  it('sends shares only where they apply', () => {
    const partial = toExpenseInput({ ...values, classification: 'partial', business_percentage: 62.5, deductible_percentage: '50' }, true);
    expect(partial).toMatchObject({ business_percentage: '62.5', deductible_percentage: '50' });
    const personal = toExpenseInput(
      { ...values, classification: 'personal', business_percentage: 62.5, deductible_percentage: '50' },
      true,
    );
    expect(personal).toMatchObject({ business_percentage: null, deductible_percentage: null });
  });

  it('validates with the API’s field names', () => {
    const schema = expenseSchema((date) => gstRegisteredOn(business, date));
    const empty = schema.safeParse(emptyExpenseValues('2026-11-01', true));
    expect(empty.error?.issues.map((i) => i.path.join('.'))).toEqual(expect.arrayContaining(['supplier', 'category_id', 'gross_amount']));
    const invalid = schema.safeParse({
      ...values,
      classification: 'partial',
      business_percentage: 40,
      deductible_percentage: 60,
      gst_override: true,
      gst_amount: 20_000,
    });
    expect(invalid.error?.issues.map((i) => i.path.join('.'))).toEqual(['gst_amount', 'deductible_percentage']);
    const recharge = schema.safeParse({ ...values, classification: 'rechargeable' });
    expect(recharge.error?.issues.map((i) => i.path.join('.'))).toEqual(['rechargeable_customer_id']);
  });

  it('leaves the deductible share empty when it equals the default, so it keeps following the default', () => {
    const expense = {
      expense_date: '2026-11-01',
      supplier: 'Spark',
      description: '',
      category_id: 'c1',
      gross_amount: 11_500,
      gst_amount: 1_500,
      gst_entered: false,
      gst_treatment: 'standard',
      payment_method: 'credit_card',
      payment_reference: '',
      classification: 'partial',
      business_percentage: '60.00',
      deductible_percentage: '50.00',
      notes: '',
    } as Expense;
    expect(fromExpense(expense, '50')).toMatchObject({ business_percentage: 60, deductible_percentage: '', gst_override: false });
    expect(fromExpense({ ...expense, deductible_percentage: '40.00', gst_amount: 1_400, gst_entered: true }, '50')).toMatchObject({
      deductible_percentage: 40,
      gst_override: true,
      gst_amount: 1_400,
    });
  });
});

describe('ExpenseFormModal', () => {
  it('validates before saving and previews how the expense is recorded', async () => {
    const fetchSpy = mockApi();
    renderWithProviders(<ExpenseFormModal opened onClose={() => undefined} />);
    await userEvent.click(await screen.findByRole('button', { name: 'Add expense' }));
    expect(await screen.findByText('Enter who you paid.')).toBeInTheDocument();
    expect(screen.getByText('Choose a category.')).toBeInTheDocument();
    expect(screen.getByText('Enter the amount paid.')).toBeInTheDocument();

    await userEvent.type(screen.getByLabelText(/^Amount paid/), '115');
    expect(await screen.findByText('How this will be recorded')).toBeInTheDocument();
    expect(screen.getByText(/GST included/)).toHaveTextContent('$15.00');
    // The figures come from the API's calculation; nothing is saved while the form is invalid.
    const calculate = fetchSpy.mock.calls.map(([r]) => r as Request).find((r) => r.url.endsWith('/expenses/calculate'));
    expect(await calculate?.json()).toMatchObject({ gross_amount: 11_500, gst_treatment: 'standard', classification: 'business' });
    expect(fetchSpy.mock.calls.every(([r]) => (r as Request).method === 'GET' || (r as Request).url.endsWith('/calculate'))).toBe(true);
  });
});
