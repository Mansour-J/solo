// The income form: values, validation (mirroring the API) and conversion to and from the API shape.
import type { Business, components } from '@solo/api-client';
import { z } from 'zod';

export type IncomeRow = components['schemas']['IncomeRow'];
export type IncomeInput = components['schemas']['IncomeInput'];
type IncomeType = components['schemas']['IncomeType'];
type TaxTreatment = components['schemas']['TaxTreatment'];
type PaymentMethod = components['schemas']['PaymentMethod'];

export interface IncomeFormValues {
  income_date: string | null;
  payer: string;
  income_type: IncomeType;
  customer_id: string | null;
  /** Minor units, including GST. */
  gross_amount: number | null;
  gst_treatment: TaxTreatment;
  /** Minor units; null lets the API calculate it. */
  gst_amount: number | null;
  payment_method: PaymentMethod;
  reference: string;
  description: string;
  notes: string;
}

export const INCOME_TYPES: { value: IncomeType; label: string }[] = [
  { value: 'sales', label: 'Sales' },
  { value: 'other', label: 'Other income' },
];

export const GST_TREATMENTS: { value: TaxTreatment; label: string }[] = [
  { value: 'standard', label: 'GST 15%' },
  { value: 'zero_rated', label: 'Zero-rated' },
  { value: 'exempt', label: 'Exempt' },
  { value: 'no_gst', label: 'No GST' },
];

export const PAYMENT_METHODS: { value: PaymentMethod; label: string }[] = [
  { value: 'bank_transfer', label: 'Bank transfer' },
  { value: 'cash', label: 'Cash' },
  { value: 'card', label: 'Card' },
  { value: 'cheque', label: 'Cheque' },
  { value: 'online', label: 'Online payment' },
  { value: 'other', label: 'Other' },
];

export function optionLabel(options: { value: string; label: string }[], value: string): string {
  return options.find((o) => o.value === value)?.label ?? value;
}

/**
 * GST in a GST-inclusive amount at 15% (3/23), rounded half away from zero like the API. Only a
 * preview: the API calculates the stored amount with the rate in force on the date.
 */
export function gstPreview(gross: number): number {
  return Math.sign(gross) * Math.round((Math.abs(gross) * 3) / 23);
}

type GstBusiness = Pick<Business, 'gst_registered' | 'gst_registered_from'>;

/** Whether GST applies on `date`: registered, and on or after the registration date. */
export function gstApplies(business: GstBusiness, date: string | null): boolean {
  if (!business.gst_registered) return false;
  if (!business.gst_registered_from || !date) return true;
  return date >= business.gst_registered_from;
}

export function emptyIncomeForm(business: GstBusiness, today: string): IncomeFormValues {
  return {
    income_date: today,
    payer: '',
    income_type: 'sales',
    customer_id: null,
    gross_amount: null,
    gst_treatment: business.gst_registered ? 'standard' : 'no_gst',
    gst_amount: null,
    payment_method: 'bank_transfer',
    reference: '',
    description: '',
    notes: '',
  };
}

export function fromIncome(row: IncomeRow): IncomeFormValues {
  // Keep a manual GST amount as an override; a calculated one is left empty so it follows the amount.
  const overridden = row.gst_treatment === 'standard' && row.gst_amount !== gstPreview(row.gross_amount);
  return {
    income_date: row.income_date,
    payer: row.payer,
    income_type: row.income_type as IncomeType,
    customer_id: row.customer_id ?? null,
    gross_amount: row.gross_amount,
    gst_treatment: row.gst_treatment,
    gst_amount: overridden ? row.gst_amount : null,
    payment_method: row.payment_method as PaymentMethod,
    reference: row.reference,
    description: row.description,
    notes: row.notes,
  };
}

export function toIncomeInput(values: IncomeFormValues, business: GstBusiness): IncomeInput {
  const gst = gstApplies(business, values.income_date);
  const treatment = gst ? values.gst_treatment : 'no_gst';
  return {
    income_date: values.income_date ?? '',
    payer: values.payer.trim(),
    income_type: values.income_type,
    customer_id: values.customer_id,
    gross_amount: values.gross_amount ?? 0,
    gst_treatment: treatment,
    gst_amount: treatment === 'standard' ? values.gst_amount : null,
    payment_method: values.payment_method,
    reference: values.reference.trim(),
    description: values.description.trim(),
    notes: values.notes.trim(),
  };
}

const maxLength = (max: number) => z.string().refine((v) => v.trim().length <= max, `Use ${max} characters or fewer.`);

/** Validation for the income form. GST checks depend on the business’s registration on the date. */
export function incomeSchema(business: GstBusiness) {
  return z
    .object({
      income_date: z.unknown().refine((v) => typeof v === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(v), 'Choose the date you were paid.'),
      payer: z.string().trim().min(1, 'Enter who paid you.').max(200, 'Use 200 characters or fewer.'),
      gross_amount: z.unknown().refine((v) => typeof v === 'number' && v > 0 && v <= 1_000_000_000_000, 'Enter the amount received.'),
      gst_amount: z.unknown(),
      gst_treatment: z.string(),
      reference: maxLength(100),
      description: maxLength(1000),
      notes: maxLength(2000),
    })
    .passthrough()
    .superRefine((values, ctx) => {
      const gst = values.gst_amount;
      const gross = values.gross_amount;
      if (
        gstApplies(business, values.income_date as string | null) &&
        values.gst_treatment === 'standard' &&
        typeof gst === 'number' &&
        typeof gross === 'number' &&
        (gst < 0 || gst > gross)
      ) {
        ctx.addIssue({ code: 'custom', path: ['gst_amount'], message: 'GST must be between 0 and the amount received.' });
      }
    });
}
