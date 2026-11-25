// Form values, validation and labels for the year-end records. Field names match the API's.
import { z } from 'zod';
import type { AssetInput, MovementInput, MovementKind, OpeningInput } from './api';

export const MOVEMENT_KINDS: { value: MovementKind; label: string; hint: string }[] = [
  { value: 'drawing', label: 'Drawing', hint: 'Money you took out of the business for yourself.' },
  { value: 'funds_introduced', label: 'Funds introduced', hint: 'Your own money you put into the business.' },
  { value: 'gst_payment', label: 'GST payment', hint: 'GST you paid to IRD for a return.' },
  { value: 'gst_refund', label: 'GST refund', hint: 'A GST refund IRD paid you.' },
  { value: 'income_tax_payment', label: 'Income tax payment', hint: 'Income tax or provisional tax you paid to IRD.' },
  { value: 'income_tax_refund', label: 'Income tax refund', hint: 'An income tax refund IRD paid you.' },
];

export function movementLabel(kind: string): string {
  return MOVEMENT_KINDS.find((k) => k.value === kind)?.label ?? kind;
}

/** Money that comes into the business account. */
export function isInflow(kind: string): boolean {
  return kind === 'funds_introduced' || kind === 'gst_refund' || kind === 'income_tax_refund';
}

// ─── Money in and out ───────────────────────────────────────────────────────

export interface MovementValues {
  movement_date: string;
  kind: MovementKind;
  amount: number | null;
  description: string;
  reference: string;
}

export const movementSchema = z
  .object({
    movement_date: z.string().min(1, 'Choose the date.'),
    amount: z.number({ error: 'Enter the amount.' }).positive('Enter an amount above zero.'),
    description: z.string().max(300, 'Use 300 characters or fewer.'),
    reference: z.string().max(100, 'Use 100 characters or fewer.'),
  })
  .passthrough();

export function movementBody(v: MovementValues): MovementInput {
  return { ...v, amount: v.amount ?? 0, description: v.description.trim(), reference: v.reference.trim() };
}

// ─── Assets ─────────────────────────────────────────────────────────────────

export interface AssetValues {
  name: string;
  asset_group: string;
  cost: number | null;
  purchase_gst: number | null;
  acquired_on: string;
  /** `full`: a low-value asset ($1,000 or less) deducted in full in the year bought. */
  method: 'dv' | 'sl' | 'full';
  rate: string;
  private_use: string;
  /** Brought forward from before the opening balances. */
  brought_forward: boolean;
  opening_value: number | null;
  notes: string;
}

const percent = (message: string, max: number, inclusive: boolean) =>
  z
    .string()
    .trim()
    .refine((v) => {
      const n = Number(v);
      return v !== '' && Number.isFinite(n) && n >= 0 && (inclusive ? n <= max : n < max);
    }, message);

export const assetSchema = z
  .object({
    name: z.string().trim().min(1, 'Enter a name.').max(120, 'Use 120 characters or fewer.'),
    asset_group: z.string().trim().min(1, 'Enter a group, e.g. Computer equipment.').max(80, 'Use 80 characters or fewer.'),
    cost: z.number({ error: 'Enter the cost excluding GST.' }).positive('Enter the cost excluding GST.'),
    acquired_on: z.string().min(1, 'Choose the date you bought it.'),
    rate: percent('Enter a rate above 0 and up to 100%.', 100, true).refine((v) => Number(v) > 0, 'Enter a rate above 0 and up to 100%.'),
    private_use: percent('Enter private use from 0 to under 100%.', 100, false),
    notes: z.string().max(1000, 'Use 1000 characters or fewer.'),
  })
  .passthrough()
  .superRefine((v, ctx) => {
    if (v.brought_forward && (v.opening_value === null || v.opening_value === undefined)) {
      ctx.addIssue({ code: 'custom', path: ['opening_value'], message: 'Enter its value at your opening balance date.' });
    }
  });

export function assetBody(v: AssetValues): AssetInput {
  return {
    name: v.name.trim(),
    asset_group: v.asset_group.trim(),
    cost: v.cost ?? 0,
    purchase_gst: v.brought_forward ? 0 : (v.purchase_gst ?? 0),
    acquired_on: v.acquired_on,
    method: v.method,
    rate: v.rate.trim(),
    private_use: v.private_use.trim() || '0',
    opening_value: v.brought_forward ? v.opening_value : null,
    notes: v.notes.trim(),
  };
}

// ─── Opening balances ───────────────────────────────────────────────────────

export interface OpeningValues {
  as_at: string;
  bank: number | null;
  gst_payable: number | null;
  drawings: number | null;
  funds_introduced: number | null;
  personal_accounts: number | null;
  withholding_tax: number | null;
  income_tax: number | null;
}

export const openingSchema = z
  .object({
    as_at: z.string().min(1, 'Choose the date of the balances, e.g. the end of your last financial year.'),
  })
  .passthrough();

/** Drawings and taxes are entered as positive amounts and sent as the balance sheet shows them (negative). */
export function openingBody(v: OpeningValues): OpeningInput {
  const n = (x: number | null) => x ?? 0;
  return {
    as_at: v.as_at,
    bank: n(v.bank),
    gst_payable: n(v.gst_payable),
    drawings: -Math.abs(n(v.drawings)),
    funds_introduced: n(v.funds_introduced),
    personal_accounts: n(v.personal_accounts),
    withholding_tax: -Math.abs(n(v.withholding_tax)),
    income_tax: -Math.abs(n(v.income_tax)),
  };
}

// ─── Years ──────────────────────────────────────────────────────────────────

/** The current financial year's end and the `count - 1` before it (YYYY-MM-DD), keeping month ends. */
export function recentYearEnds(currentEnd: string, count = 6): string[] {
  const [y, m, d] = currentEnd.split('-').map(Number) as [number, number, number];
  const lastDay = (year: number) => new Date(Date.UTC(year, m, 0)).getUTCDate();
  const monthEnd = d === lastDay(y);
  return Array.from({ length: count }, (_, n) => {
    const year = y - n;
    const day = monthEnd ? lastDay(year) : Math.min(d, lastDay(year));
    return `${year}-${String(m).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
  });
}

/** Assets costing over this (excluding GST, minor units) may need GST change-of-use reviews; the API decides. */
export const CHANGE_OF_USE_FROM = 1_000_000;

/** The tabs of the Year end page (`/year-end?tab=…`); the guide covers each one (guide.test.tsx). */
export const YEAR_END_TABS = ['money', 'assets', 'home-office', 'opening'] as const;
