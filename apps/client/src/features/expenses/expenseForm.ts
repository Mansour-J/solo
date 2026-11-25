import { z } from 'zod';
import type { Expense, ExpenseInput, PaymentMethod } from './api';
import {
  businessShareHundredths,
  defaultDeductibleHundredths,
  hundredthsToString,
  percentHundredths,
  type Classification,
  type TaxTreatment,
} from './expenseMath';

export const PAYMENT_METHOD_LABELS: Record<PaymentMethod, string> = {
  business_account: 'Business account',
  personal_funds: 'My own money',
  credit_card: 'Credit card',
  cash: 'Cash',
  other: 'Other',
};

export const PAYMENT_METHOD_OPTIONS = (Object.keys(PAYMENT_METHOD_LABELS) as PaymentMethod[]).map((value) => ({
  value,
  label: PAYMENT_METHOD_LABELS[value],
}));

export const CLASSIFICATION_LABELS: Record<Classification, string> = {
  business: 'Business',
  partial: 'Partly business',
  personal: 'Personal',
  rechargeable: 'Recharge to a customer',
};

export const CLASSIFICATION_DESCRIPTIONS: Record<Classification, string> = {
  business: 'Entirely for the business.',
  partial: 'Used for both business and private purposes.',
  personal: 'Private spending. Recorded as drawings, not a business cost.',
  rechargeable: 'A business cost you’ll pass on to a customer.',
};

export const CLASSIFICATIONS: Classification[] = ['business', 'partial', 'personal', 'rechargeable'];

export const GST_TREATMENT_LABELS: Record<TaxTreatment, string> = {
  standard: 'Includes GST (15%)',
  zero_rated: 'Zero-rated (0%)',
  exempt: 'Exempt from GST',
  no_gst: 'No GST',
};

export const GST_TREATMENT_OPTIONS = (Object.keys(GST_TREATMENT_LABELS) as TaxTreatment[]).map((value) => ({
  value,
  label: GST_TREATMENT_LABELS[value],
}));

/** Form values. Field names match `ExpenseInput` so server errors land on the right input. */
export interface ExpenseFormValues {
  expense_date: string | null;
  supplier: string;
  description: string;
  category_id: string | null;
  gross_amount: number | null;
  gst_treatment: TaxTreatment;
  /** Not sent: whether the GST shown on the receipt is entered instead of calculated. */
  gst_override: boolean;
  gst_amount: number | null;
  payment_method: PaymentMethod;
  payment_reference: string;
  classification: Classification;
  business_percentage: number | string;
  /** Empty to use the default (the business share, capped by the business’s default). */
  deductible_percentage: number | string;
  rechargeable_customer_id: string | null;
  notes: string;
}

export function emptyExpenseValues(today: string, registered: boolean): ExpenseFormValues {
  return {
    expense_date: today,
    supplier: '',
    description: '',
    category_id: null,
    gross_amount: null,
    gst_treatment: registered ? 'standard' : 'no_gst',
    gst_override: false,
    gst_amount: null,
    payment_method: 'business_account',
    payment_reference: '',
    classification: 'business',
    business_percentage: '',
    deductible_percentage: '',
    rechargeable_customer_id: null,
    notes: '',
  };
}

function toNumberOrEmpty(value: string): number | string {
  const hundredths = percentHundredths(value);
  return hundredths === null ? '' : hundredths / 100;
}

/**
 * Values for editing an expense. The deductible share stays empty when it equals the default, so it
 * keeps following the business share if that changes.
 */
export function fromExpense(expense: Expense, businessDefault: string | null | undefined): ExpenseFormValues {
  const classification = expense.classification as Classification;
  const share = businessShareHundredths(classification, expense.business_percentage);
  const deductible = percentHundredths(expense.deductible_percentage);
  const usesDefault = deductible === defaultDeductibleHundredths(classification, share, businessDefault);
  return {
    expense_date: expense.expense_date,
    supplier: expense.supplier,
    description: expense.description,
    category_id: expense.category_id,
    gross_amount: expense.gross_amount,
    gst_treatment: expense.gst_treatment,
    gst_override: expense.gst_entered,
    gst_amount: expense.gst_treatment === 'standard' ? expense.gst_amount : null,
    payment_method: expense.payment_method as PaymentMethod,
    payment_reference: expense.payment_reference,
    classification,
    business_percentage: classification === 'partial' ? toNumberOrEmpty(expense.business_percentage) : '',
    deductible_percentage: usesDefault ? '' : toNumberOrEmpty(expense.deductible_percentage),
    rechargeable_customer_id: expense.rechargeable_customer_id ?? null,
    notes: expense.notes,
  };
}

const isEmpty = (value: number | string) => String(value).trim() === '';

/** Client-side checks (the API repeats them). `registeredOn` tells whether GST applies on a date. */
export function expenseSchema(registeredOn: (date: string | null) => boolean) {
  return z
    .object({
      expense_date: z
        .string()
        .nullable()
        .refine((v) => !!v, 'Enter the date.'),
      supplier: z.string().trim().min(1, 'Enter who you paid.').max(200, 'Use 200 characters or fewer.'),
      description: z.string().max(1000, 'Use 1,000 characters or fewer.'),
      category_id: z
        .string()
        .nullable()
        .refine((v) => !!v, 'Choose a category.'),
      gross_amount: z
        .number()
        .nullable()
        .refine((v) => v !== null && v > 0, 'Enter the amount paid.'),
      payment_reference: z.string().max(100, 'Use 100 characters or fewer.'),
      notes: z.string().max(2000, 'Use 2,000 characters or fewer.'),
    })
    .loose()
    .superRefine((raw, ctx) => {
      const v = raw as unknown as ExpenseFormValues;
      if (registeredOn(v.expense_date) && v.gst_treatment === 'standard' && v.gst_override) {
        if (v.gst_amount === null) {
          ctx.addIssue({ code: 'custom', path: ['gst_amount'], message: 'Enter the GST shown on the receipt.' });
        } else if (v.gross_amount !== null && (v.gst_amount < 0 || v.gst_amount > v.gross_amount)) {
          ctx.addIssue({ code: 'custom', path: ['gst_amount'], message: 'GST must be between 0 and the amount paid.' });
        }
      }
      const share = businessShareHundredths(v.classification, v.business_percentage);
      if (v.classification === 'partial' && share === null) {
        ctx.addIssue({ code: 'custom', path: ['business_percentage'], message: 'Enter the business share (between 0 and 100%).' });
      }
      if (v.classification !== 'personal' && !isEmpty(v.deductible_percentage)) {
        const deductible = percentHundredths(v.deductible_percentage);
        if (deductible === null) {
          ctx.addIssue({ code: 'custom', path: ['deductible_percentage'], message: 'Enter a percentage between 0 and 100.' });
        } else if (share !== null && deductible > share) {
          ctx.addIssue({
            code: 'custom',
            path: ['deductible_percentage'],
            message: 'The deductible share can’t be more than the business share.',
          });
        }
      }
      if (v.classification === 'rechargeable' && !v.rechargeable_customer_id) {
        ctx.addIssue({ code: 'custom', path: ['rechargeable_customer_id'], message: 'Choose the customer to recharge.' });
      }
    });
}

function percentString(value: number | string): string | null {
  const hundredths = percentHundredths(value);
  return hundredths === null ? null : hundredthsToString(hundredths);
}

/** The request body. Without GST registration on the date, GST isn't claimable: send `no_gst`. */
export function toExpenseInput(values: ExpenseFormValues, registered: boolean): ExpenseInput {
  const standard = registered && values.gst_treatment === 'standard';
  return {
    expense_date: values.expense_date ?? '',
    supplier: values.supplier.trim(),
    description: values.description.trim(),
    category_id: values.category_id ?? '',
    gross_amount: values.gross_amount ?? 0,
    gst_treatment: registered ? values.gst_treatment : 'no_gst',
    gst_amount: standard && values.gst_override ? values.gst_amount : null,
    payment_method: values.payment_method,
    payment_reference: values.payment_reference.trim(),
    classification: values.classification,
    business_percentage: values.classification === 'partial' ? percentString(values.business_percentage) : null,
    deductible_percentage:
      values.classification !== 'personal' && !isEmpty(values.deductible_percentage) ? percentString(values.deductible_percentage) : null,
    rechargeable_customer_id: values.classification === 'rechargeable' ? values.rechargeable_customer_id : null,
    notes: values.notes.trim(),
  };
}
