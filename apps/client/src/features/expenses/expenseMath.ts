// Parsing and display helpers for the expense form. Amounts (GST, the business and private split)
// come from the API (`useExpenseCalculation`): nothing here calculates money.
import type { components } from '@solo/api-client';

export type TaxTreatment = components['schemas']['TaxTreatment'];
export type Classification = components['schemas']['Classification'];
export type ExpenseSplit = components['schemas']['ExpenseSplit'];

/**
 * A percentage ("62.5", 62.5, "100.00") as hundredths of a percent (6250), or null when it isn't a
 * number between 0 and 100 with at most two decimal places.
 */
export function percentHundredths(value: string | number | null | undefined): number | null {
  if (value === null || value === undefined) return null;
  const text = String(value).trim();
  const match = /^(\d{1,3})(?:\.(\d{0,2}))?$/.exec(text);
  if (!match) return null;
  const hundredths = Number(match[1]) * 100 + Number((match[2] ?? '').padEnd(2, '0'));
  return hundredths <= 10_000 ? hundredths : null;
}

/** Whether the business is registered for GST on `date` (YYYY-MM-DD). */
export function gstRegisteredOn(
  business: { gst_registered: boolean; gst_registered_from?: string | null },
  date: string | null | undefined,
): boolean {
  if (!business.gst_registered) return false;
  if (!business.gst_registered_from || !date) return true;
  return date >= business.gst_registered_from;
}

/** The business share implied by a classification, in hundredths; null when a partial share is invalid. */
export function businessShareHundredths(
  classification: Classification,
  businessPercentage: string | number | null | undefined,
): number | null {
  switch (classification) {
    case 'business':
    case 'rechargeable':
      return 10_000;
    case 'personal':
      return 0;
    case 'partial': {
      const share = percentHundredths(businessPercentage);
      return share !== null && share > 0 && share < 10_000 ? share : null;
    }
  }
}

/** The deductible share the API applies when none is entered: the business share, capped by the default. */
export function defaultDeductibleHundredths(
  classification: Classification,
  businessShare: number | null,
  businessDefault: string | null | undefined,
): number | null {
  if (classification === 'personal') return 0;
  const cap = percentHundredths(businessDefault);
  if (businessShare === null || cap === null) return null;
  return Math.min(businessShare, cap);
}

/** Hundredths of a percent as a plain string for the API and labels, e.g. 6250 → "62.5". */
export function hundredthsToString(hundredths: number): string {
  const whole = Math.trunc(hundredths / 100);
  const fraction = String(hundredths % 100)
    .padStart(2, '0')
    .replace(/0+$/, '');
  return fraction ? `${whole}.${fraction}` : String(whole);
}

/** A percentage from the API ("62.50") for display, e.g. "62.5%". */
export function formatShare(value: string | number | null | undefined): string {
  const hundredths = percentHundredths(value);
  return hundredths === null ? '—' : `${hundredthsToString(hundredths)}%`;
}
