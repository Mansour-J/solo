// Shared editor model for invoices and quotes. Amounts stay as strings or minor units in the form;
// totals always come from the API's calculation endpoint.
import dayjs from 'dayjs';
import { z } from 'zod';
import type { Invoice, LineInput, Quote, TaxTreatment } from './api';

export interface LineValues {
  key: string;
  description: string;
  quantity: string;
  unit_price: number | null;
  discount_percent: string;
  tax_treatment: TaxTreatment;
}

export interface DocumentValues {
  customer_id: string | null;
  issue_date: string;
  /** Due date (invoices) or expiry date (quotes). Empty = calculated from terms by the API. */
  end_date: string | null;
  reference: string;
  prices_include_gst: boolean;
  notes: string;
  footer: string;
  lines: LineValues[];
}

export const TAX_TREATMENTS: { value: TaxTreatment; label: string }[] = [
  { value: 'standard', label: 'GST 15%' },
  { value: 'zero_rated', label: 'Zero-rated' },
  { value: 'exempt', label: 'Exempt' },
  { value: 'no_gst', label: 'No GST' },
];

let lineCounter = 0;

export function emptyLine(gstRegistered: boolean): LineValues {
  lineCounter += 1;
  return {
    key: `line-${lineCounter}`,
    description: '',
    quantity: '1',
    unit_price: null,
    discount_percent: '0',
    tax_treatment: gstRegistered ? 'standard' : 'no_gst',
  };
}

export function newDocument(today: string, gstRegistered: boolean, customerId: string | null, notes = '', footer = ''): DocumentValues {
  return {
    customer_id: customerId,
    issue_date: today,
    end_date: null,
    reference: '',
    prices_include_gst: false,
    notes,
    footer,
    lines: [emptyLine(gstRegistered)],
  };
}

export function fromDocument(doc: Invoice | Quote): DocumentValues {
  return {
    customer_id: doc.customer.id,
    issue_date: doc.issue_date,
    end_date: 'due_date' in doc ? doc.due_date : doc.expiry_date,
    reference: doc.reference,
    prices_include_gst: doc.prices_include_gst,
    notes: doc.notes,
    footer: doc.footer,
    lines: doc.lines.map((l) => {
      lineCounter += 1;
      return {
        key: `line-${lineCounter}`,
        description: l.description,
        quantity: l.quantity,
        unit_price: l.unit_price,
        discount_percent: l.discount_percent,
        tax_treatment: l.tax_treatment,
      };
    }),
  };
}

const decimal = /^\d+(\.\d{1,4})?$/;

/** Lines the API can calculate (complete ones); incomplete lines are left out of the preview. */
export function calculableLines(lines: LineValues[]): LineInput[] {
  return lines
    .filter((l) => l.unit_price !== null && decimal.test(l.quantity.trim()) && Number(l.quantity) > 0)
    .map((l) => ({
      description: l.description.trim() || 'Item',
      quantity: l.quantity.trim(),
      unit_price: l.unit_price ?? 0,
      discount_percent: l.discount_percent.trim() || '0',
      tax_treatment: l.tax_treatment,
    }));
}

export function toLines(lines: LineValues[]): LineInput[] {
  return lines.map((l) => ({
    description: l.description.trim(),
    quantity: l.quantity.trim(),
    unit_price: l.unit_price ?? 0,
    discount_percent: l.discount_percent.trim() || '0',
    tax_treatment: l.tax_treatment,
  }));
}

export const documentSchema = z
  .object({
    customer_id: z.string({ error: 'Choose a customer.' }).min(1, 'Choose a customer.'),
    issue_date: z.string().min(1, 'Choose a date.'),
    reference: z.string().max(100, 'Use 100 characters or fewer.'),
    notes: z.string().max(4000, 'Use 4000 characters or fewer.'),
    footer: z.string().max(2000, 'Use 2000 characters or fewer.'),
    lines: z
      .array(
        z
          .object({
            description: z.string().trim().min(1, 'Describe the item.').max(1000),
            quantity: z
              .string()
              .regex(decimal, 'Enter a quantity, e.g. 1 or 7.5.')
              .refine((q) => Number(q) > 0, 'Enter a quantity above 0.'),
            unit_price: z.number({ error: 'Enter a price.' }).min(0, 'Enter a price of 0 or more.'),
            discount_percent: z
              .string()
              .regex(/^\d{1,3}(\.\d{1,2})?$/, 'Use a percentage, e.g. 10.')
              .refine((d) => Number(d) <= 100, 'Use 100% or less.'),
          })
          .passthrough(),
      )
      .min(1, 'Add at least one line.'),
  })
  .passthrough()
  .refine((v) => !v.end_date || !v.issue_date || !dayjs(v.end_date as string).isBefore(dayjs(v.issue_date)), {
    path: ['end_date'],
    message: 'This can’t be before the issue date.',
  });
