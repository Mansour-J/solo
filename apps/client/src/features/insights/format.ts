// Display helpers for insights (every figure comes from the API).
import { formatMoney, formatNumber } from '@/lib/format';
import type { InsightFigure, Tone } from './api';

export function formatFigure(figure: InsightFigure, currency: string): string {
  switch (figure.kind) {
    case 'money':
      return formatMoney(figure.amount, currency);
    case 'percent':
      return `${formatNumber(figure.amount)}%`;
    case 'days':
      return `${formatNumber(figure.amount)} ${figure.amount === 1 ? 'day' : 'days'}`;
    default:
      return formatNumber(figure.amount);
  }
}

/** Mantine colour per tone; always paired with words, never the only signal. */
export const TONE_COLOURS: Record<Tone, string> = {
  positive: 'teal',
  negative: 'red',
  warning: 'orange',
  neutral: 'gray',
};
