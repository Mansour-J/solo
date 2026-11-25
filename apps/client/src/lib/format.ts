// Formatting for dates, money and numbers. Money arrives from the API as integer minor units.
import dayjs from 'dayjs';
import relativeTime from 'dayjs/plugin/relativeTime';
import type { Preferences } from '@solo/api-client';

dayjs.extend(relativeTime);

export type DateFormatPreference = Preferences['date_format'];

const DAYJS_FORMATS: Record<DateFormatPreference, string> = {
  'DD/MM/YYYY': 'DD/MM/YYYY',
  'YYYY-MM-DD': 'YYYY-MM-DD',
  'MM/DD/YYYY': 'MM/DD/YYYY',
};

let dateFormat: DateFormatPreference = 'DD/MM/YYYY';
let numberLocale = 'en-NZ';

/** Applies the user's formatting preferences (called when the profile loads or changes). */
export function setFormatPreferences(preferences: Pick<Preferences, 'date_format' | 'number_locale'>) {
  dateFormat = preferences.date_format;
  numberLocale = preferences.number_locale;
}

/** The user's number locale (e.g. `en-NZ`), for formatting that isn't covered here. */
export function getNumberLocale(): string {
  return numberLocale;
}

export function dateInputFormat(): string {
  return DAYJS_FORMATS[dateFormat];
}

/** A calendar date (`2026-11-20`) or timestamp, in the user's date format. */
export function formatDate(value: string | Date | null | undefined): string {
  if (!value) return '—';
  const date = dayjs(value);
  return date.isValid() ? date.format(DAYJS_FORMATS[dateFormat]) : '—';
}

export function formatDateTime(value: string | Date | null | undefined): string {
  if (!value) return '—';
  const date = dayjs(value);
  return date.isValid() ? `${date.format(DAYJS_FORMATS[dateFormat])} ${date.format('h:mm a')}` : '—';
}

export function formatRelative(value: string | Date): string {
  return dayjs(value).fromNow();
}

/** Minor-unit digits per currency (matches the API's currency table). */
const MINOR_UNITS: Record<string, number> = { JPY: 0 };

export function minorUnits(currency: string): number {
  return MINOR_UNITS[currency] ?? 2;
}

/** Formats an amount in minor units, e.g. 123456 NZD → "$1,234.56". */
export function formatMoney(minor: number, currency = 'NZD', options: { signDisplay?: 'auto' | 'always' | 'exceptZero' } = {}): string {
  const digits = minorUnits(currency);
  return new Intl.NumberFormat(numberLocale, {
    style: 'currency',
    currency,
    currencyDisplay: 'narrowSymbol',
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
    signDisplay: options.signDisplay ?? 'auto',
  }).format(minor / 10 ** digits);
}

/** Compact money for chart axes and KPI cards, e.g. "$12.3k". */
export function formatMoneyCompact(minor: number, currency = 'NZD'): string {
  const digits = minorUnits(currency);
  return new Intl.NumberFormat(numberLocale, {
    style: 'currency',
    currency,
    currencyDisplay: 'narrowSymbol',
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(minor / 10 ** digits);
}

/**
 * Parses user input ("1,234.5", "$20") into minor units without floating-point arithmetic.
 * Returns null for anything that isn't a number with at most the currency's decimal places.
 */
export function parseMoney(input: string | number, currency = 'NZD'): number | null {
  const digits = minorUnits(currency);
  const text = String(input).replace(/[\s,$£€¥]/g, '');
  const match = /^(-)?(\d*)(?:\.(\d*))?$/.exec(text);
  if (!match || (!match[2] && !match[3])) return null;
  const [, sign, whole = '', fraction = ''] = match;
  if (fraction.length > digits) return null;
  const minor = Number(whole || '0') * 10 ** digits + Number(fraction.padEnd(digits, '0') || '0');
  if (!Number.isSafeInteger(minor)) return null;
  return sign ? -minor : minor;
}

/** Minor units as a plain decimal string for inputs, e.g. 1234 → "12.34". */
export function minorToDecimalString(minor: number, currency = 'NZD'): string {
  const digits = minorUnits(currency);
  if (digits === 0) return String(minor);
  const sign = minor < 0 ? '-' : '';
  const abs = Math.abs(minor);
  return `${sign}${Math.floor(abs / 10 ** digits)}.${String(abs % 10 ** digits).padStart(digits, '0')}`;
}

export function formatPercent(value: number, fractionDigits = 0): string {
  return new Intl.NumberFormat(numberLocale, { style: 'percent', maximumFractionDigits: fractionDigits }).format(value);
}

export function formatNumber(value: number, fractionDigits = 0): string {
  return new Intl.NumberFormat(numberLocale, { maximumFractionDigits: fractionDigits }).format(value);
}

const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];

export function monthName(month: number): string {
  return MONTHS[month - 1] ?? '';
}

/** NZ GST/IRD numbers are written in groups of three: 49-091-850, 136-410-132. */
export function formatGstNumber(digits: string): string {
  const padded = digits.padStart(9, '0');
  const groups = [padded.slice(0, 3), padded.slice(3, 6), padded.slice(6)];
  if (digits.length === 8) groups[0] = groups[0]!.slice(1);
  return groups.join('-');
}
