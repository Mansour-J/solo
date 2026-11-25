// The business profile form: values, validation and conversion to and from the API shape.
// Used by onboarding and the business settings page.
import type { Business, BusinessInput, BusinessType } from '@solo/api-client';
import { z } from 'zod';

export const BUSINESS_TYPES: { value: BusinessType; label: string }[] = [
  { value: 'sole_trader', label: 'Sole trader' },
  { value: 'contractor', label: 'Contractor' },
  { value: 'freelancer', label: 'Freelancer' },
  { value: 'partnership', label: 'Partnership' },
  { value: 'company', label: 'Company' },
  { value: 'other', label: 'Other' },
];

export const GST_BASES = [
  { value: 'payments', label: 'Payments basis', description: 'GST when money is received or paid (most small businesses).' },
  { value: 'invoice', label: 'Invoice basis', description: 'GST when invoices are issued or received.' },
  { value: 'hybrid', label: 'Hybrid basis', description: 'Invoice basis for sales, payments basis for purchases.' },
] as const;

export const GST_PERIODS = [
  { value: 'two_monthly', label: 'Every two months' },
  { value: 'monthly', label: 'Every month' },
  { value: 'six_monthly', label: 'Every six months' },
];

export const MONTH_OPTIONS = Array.from({ length: 12 }, (_, i) => ({
  value: String(i + 1),
  label: new Date(2000, i, 1).toLocaleString('en-NZ', { month: 'long' }),
}));

export const DAY_OPTIONS = Array.from({ length: 28 }, (_, i) => ({ value: String(i + 1), label: String(i + 1) }));

export interface BusinessFormValues {
  name: string;
  trading_name: string;
  business_type: BusinessType;
  email: string;
  phone: string;
  address: { line1: string; line2: string; suburb: string; city: string; region: string; postcode: string; country: string };
  nzbn: string;
  gst_registered: boolean;
  gst_number: string;
  gst_registered_from: string | null;
  gst_basis: 'invoice' | 'payments' | 'hybrid';
  gst_period: 'monthly' | 'two_monthly' | 'six_monthly';
  gst_period_end_month: string;
  currency: string;
  financial_year_start_month: string;
  financial_year_start_day: string;
  reporting_basis: 'cash' | 'accrual';
}

export const emptyBusinessForm: BusinessFormValues = {
  name: '',
  trading_name: '',
  business_type: 'sole_trader',
  email: '',
  phone: '',
  address: { line1: '', line2: '', suburb: '', city: '', region: '', postcode: '', country: 'NZ' },
  nzbn: '',
  gst_registered: false,
  gst_number: '',
  gst_registered_from: null,
  gst_basis: 'payments',
  gst_period: 'two_monthly',
  gst_period_end_month: '3',
  currency: 'NZD',
  financial_year_start_month: '4',
  financial_year_start_day: '1',
  reporting_basis: 'accrual',
};

const digits = (value: string) => value.replace(/[\s-]/g, '');

export const businessSchema = z
  .object({
    name: z.string().trim().min(1, 'Enter your business name.').max(200),
    trading_name: z.string().max(200),
    email: z.union([z.literal(''), z.string().trim().email('Enter a valid email address.')]),
    phone: z
      .string()
      .max(40)
      .regex(/^[\d\s+()\-.]*$/, 'Use digits, spaces and + ( ) - only.'),
    nzbn: z.string().refine((v) => v.trim() === '' || /^\d{13}$/.test(digits(v)), 'An NZBN has 13 digits.'),
    gst_registered: z.boolean(),
    gst_number: z.string(),
  })
  .passthrough()
  .refine((v) => !v.gst_registered || v.gst_number.trim() === '' || /^\d{8,9}$/.test(digits(v.gst_number)), {
    path: ['gst_number'],
    message: 'A GST number has 8 or 9 digits.',
  });

export function toBusinessInput(values: BusinessFormValues, completeOnboarding = false): BusinessInput {
  return {
    name: values.name.trim(),
    trading_name: values.trading_name.trim(),
    business_type: values.business_type,
    email: values.email.trim() || null,
    phone: values.phone.trim() || null,
    address: values.address,
    nzbn: values.nzbn.trim() || null,
    gst_registered: values.gst_registered,
    gst_number: values.gst_registered ? values.gst_number.trim() || null : null,
    gst_registered_from: values.gst_registered ? values.gst_registered_from : null,
    gst_basis: values.gst_basis,
    gst_period: values.gst_period,
    gst_period_end_month: Number(values.gst_period_end_month),
    currency: values.currency,
    financial_year_start_month: Number(values.financial_year_start_month),
    financial_year_start_day: Number(values.financial_year_start_day),
    reporting_basis: values.reporting_basis,
    complete_onboarding: completeOnboarding,
  };
}

export function fromBusiness(business: Business): BusinessFormValues {
  return {
    name: business.name,
    trading_name: business.trading_name,
    business_type: business.business_type,
    email: business.email ?? '',
    phone: business.phone ?? '',
    address: { ...emptyBusinessForm.address, ...business.address },
    nzbn: business.nzbn ?? '',
    gst_registered: business.gst_registered,
    gst_number: business.gst_number ?? '',
    gst_registered_from: business.gst_registered_from ?? null,
    gst_basis: business.gst_basis,
    gst_period: business.gst_period,
    gst_period_end_month: String(business.gst_period_end_month),
    currency: business.currency,
    financial_year_start_month: String(business.financial_year_start_month),
    financial_year_start_day: String(business.financial_year_start_day),
    reporting_basis: business.reporting_basis,
  };
}
