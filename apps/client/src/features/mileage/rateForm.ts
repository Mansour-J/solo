// Mileage rate forms: values, validation (mirroring the API) and conversion to the API shape.
// Rates are the user's own figures: nothing here supplies or suggests a government rate.
import type { components } from '@solo/api-client';
import { z } from 'zod';
import { formatDate } from '@/lib/format';
import type { MileageRate, RateVersion } from './api';
import { formatRate } from './format';

export type NewRateInput = components['schemas']['NewRateInput'];
export type RateInput = components['schemas']['RateInput'];
export type RateVersionInput = components['schemas']['RateVersionInput'];

/** The name a new rate starts with, when no active rate has it yet. */
export const DEFAULT_RATE_NAME = 'Kilometre rate';

const RATE_MESSAGE = 'Enter a rate between 0 and 100 per kilometre, with up to 4 decimal places.';

/**
 * A rate per kilometre as typed ("1.04", "$0.825", "2") → the decimal string the API takes ("1.04",
 * "0.825", "2"), or null unless it's between 0 and 100 with at most 4 decimal places. Works on the
 * digits, never on floating-point numbers.
 */
export function parseRatePerKm(input: string): string | null {
  const text = input.trim().replace(/^\$\s*/, '');
  const match = /^(\d*)(?:\.(\d{0,4}))?$/.exec(text);
  if (!match || (!match[1] && !match[2])) return null;
  const whole = (match[1] || '0').replace(/^0+(?=\d)/, '');
  const fraction = (match[2] ?? '').replace(/0+$/, '');
  if (whole.length > 3 || Number(whole) > 100 || (whole === '100' && fraction !== '')) return null;
  return fraction ? `${whole}.${fraction}` : whole;
}

/** An API rate ("1.0400") for an input: trailing zeros dropped, at least 2 decimals ("1.04", "0.825"). */
export function rateInputValue(rate: string): string {
  const [whole = '0', fraction = ''] = rate.split('.');
  return `${whole}.${fraction.replace(/0+$/, '').padEnd(2, '0')}`;
}

const nameField = z.string().trim().min(1, 'Enter a name for the rate.').max(80, 'Use 80 characters or fewer.');
const notesField = z.string().refine((v) => v.trim().length <= 1000, 'Use 1,000 characters or fewer.');
const effectiveFromField = z
  .string()
  .nullable()
  .refine((v) => !!v, 'Enter the date the rate applies from.');
/** A tier 1 limit as typed ("14,000", "14000.5") → "14000", or null unless more than 0 km. */
export function parseLimitKm(input: string): string | null {
  const text = input
    .trim()
    .replace(/,/g, '')
    .replace(/\s*km$/i, '');
  const match = /^(\d+)(?:\.(\d))?$/.exec(text);
  if (!match) return null;
  const whole = match[1]!.replace(/^0+(?=\d)/, '');
  const value = match[2] && match[2] !== '0' ? `${whole}.${match[2]}` : whole;
  return Number(value) > 0 && Number(value) <= 9_999_999 ? value : null;
}

/** Optional tiers (IRD's kilometre rates): a tier 1 limit and a tier 2 rate, both or neither. */
export interface TierValues {
  tier1_limit_km?: string;
  tier2_rate_per_km?: string;
}

const tierFields = { tier1_limit_km: z.string().optional(), tier2_rate_per_km: z.string().optional() };

function checkTiers(values: TierValues, ctx: z.RefinementCtx) {
  const limit = (values.tier1_limit_km ?? '').trim();
  const rate = (values.tier2_rate_per_km ?? '').trim();
  if (limit === '' && rate === '') return;
  if (limit === '')
    ctx.addIssue({ code: 'custom', path: ['tier1_limit_km'], message: 'Enter the tier 1 limit, or clear the tier 2 rate.' });
  else if (parseLimitKm(limit) === null)
    ctx.addIssue({ code: 'custom', path: ['tier1_limit_km'], message: 'Enter a limit of more than 0 km.' });
  if (rate === '')
    ctx.addIssue({ code: 'custom', path: ['tier2_rate_per_km'], message: 'Enter the tier 2 rate, or clear the tier 1 limit.' });
  else if (parseRatePerKm(rate) === null) ctx.addIssue({ code: 'custom', path: ['tier2_rate_per_km'], message: RATE_MESSAGE });
}

/** The API's tier fields, only when they're filled in. */
function tierInput(values: TierValues): { tier1_limit_km?: string; tier2_rate_per_km?: string } {
  const limit = (values.tier1_limit_km ?? '').trim();
  const rate = (values.tier2_rate_per_km ?? '').trim();
  if (limit === '' && rate === '') return {};
  return { tier1_limit_km: parseLimitKm(limit) ?? limit, tier2_rate_per_km: parseRatePerKm(rate) ?? rate };
}

const ratePerKmField = z.string().superRefine((value, ctx) => {
  if (value.trim() === '') ctx.addIssue({ code: 'custom', message: 'Enter the rate per kilometre.' });
  else if (parseRatePerKm(value) === null) ctx.addIssue({ code: 'custom', message: RATE_MESSAGE });
});

// ─── A new rate (with its first version) ────────────────────────────────────

export interface NewRateValues extends TierValues {
  name: string;
  notes: string;
  effective_from: string | null;
  rate_per_km: string;
}

export const newRateSchema = z
  .object({ name: nameField, notes: notesField, effective_from: effectiveFromField, rate_per_km: ratePerKmField, ...tierFields })
  .loose()
  .superRefine(checkTiers);

/** Empty apart from the name and the first day (usually the start of the financial year). */
export function emptyNewRateValues(effectiveFrom: string | null, name = DEFAULT_RATE_NAME): NewRateValues {
  return { name, notes: '', effective_from: effectiveFrom, rate_per_km: '', tier1_limit_km: '', tier2_rate_per_km: '' };
}

/** "Kilometre rate", unless an active rate already has that name. */
export function defaultRateName(rates: MileageRate[]): string {
  const taken = rates.some((r) => !r.archived && r.name.trim().toLowerCase() === DEFAULT_RATE_NAME.toLowerCase());
  return taken ? '' : DEFAULT_RATE_NAME;
}

export function toNewRateInput(values: NewRateValues): NewRateInput {
  return {
    name: values.name.trim(),
    notes: values.notes.trim(),
    effective_from: values.effective_from ?? '',
    rate_per_km: parseRatePerKm(values.rate_per_km) ?? values.rate_per_km.trim(),
    ...tierInput(values),
  };
}

// ─── A rate's name and notes ────────────────────────────────────────────────

export interface RateValues {
  name: string;
  notes: string;
}

export const rateSchema = z.object({ name: nameField, notes: notesField }).loose();

export function fromRate(rate: MileageRate): RateValues {
  return { name: rate.name, notes: rate.notes };
}

export function toRateInput(values: RateValues): RateInput {
  return { name: values.name.trim(), notes: values.notes.trim() };
}

// ─── A dated version of a rate ──────────────────────────────────────────────

export interface RateVersionValues extends TierValues {
  effective_from: string | null;
  rate_per_km: string;
}

export const rateVersionSchema = z
  .object({ effective_from: effectiveFromField, rate_per_km: ratePerKmField, ...tierFields })
  .loose()
  .superRefine(checkTiers);

export function fromVersion(version: RateVersion): RateVersionValues {
  return {
    effective_from: version.effective_from,
    rate_per_km: rateInputValue(version.rate_per_km),
    tier1_limit_km: version.tier1_limit_km ? String(Number(version.tier1_limit_km)) : '',
    tier2_rate_per_km: version.tier2_rate_per_km ? rateInputValue(version.tier2_rate_per_km) : '',
  };
}

/** A new version starts on the first day of the financial year, or today if that date is taken. */
export function newVersionValues(rate: MileageRate, financialYearStart: string, today: string): RateVersionValues {
  const taken = new Set(rate.versions.map((v) => v.effective_from));
  const date = [financialYearStart, today].find((d) => !taken.has(d)) ?? null;
  return { effective_from: date, rate_per_km: '', tier1_limit_km: '', tier2_rate_per_km: '' };
}

export function toRateVersionInput(values: RateVersionValues): RateVersionInput {
  return {
    effective_from: values.effective_from ?? '',
    rate_per_km: parseRatePerKm(values.rate_per_km) ?? values.rate_per_km.trim(),
    ...tierInput(values),
  };
}

// ─── Display ────────────────────────────────────────────────────────────────

/** The version that describes a rate: the one in force today, else its first (future) one. */
export function displayVersion(rate: MileageRate): RateVersion | null {
  return rate.current ?? rate.versions[0] ?? null;
}

/** "Kilometre rate · $1.04/km from 01/04/2026" (with "(archived)" for an archived rate). */
export function rateOptionLabel(rate: MileageRate, currency: string): string {
  const version = displayVersion(rate);
  const label = version
    ? `${rate.name} · ${formatRate(version.rate_per_km, currency)} from ${formatDate(version.effective_from)}`
    : rate.name;
  return rate.archived ? `${label} (archived)` : label;
}
