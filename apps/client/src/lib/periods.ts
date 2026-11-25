// Date ranges for filters and the dashboard, based on the business's financial year.
import dayjs from 'dayjs';

export type PeriodKey = 'this_month' | 'last_month' | 'this_fy' | 'last_fy' | 'last_12_months' | 'custom';

export interface Period {
  key: PeriodKey;
  from: string;
  to: string;
}

export const PERIOD_LABELS: Record<PeriodKey, string> = {
  this_month: 'This month',
  last_month: 'Last month',
  this_fy: 'This financial year',
  last_fy: 'Last financial year',
  last_12_months: 'Last 12 months',
  custom: 'Custom range',
};

const iso = (d: dayjs.Dayjs) => d.format('YYYY-MM-DD');

/** `today` is the business's today (YYYY-MM-DD); `fyStart` its current financial year start. */
export function periodRange(key: Exclude<PeriodKey, 'custom'>, today: string, fyStart: string): Period {
  const t = dayjs(today);
  const fy = dayjs(fyStart);
  switch (key) {
    case 'this_month':
      return { key, from: iso(t.startOf('month')), to: iso(t.endOf('month')) };
    case 'last_month': {
      const m = t.subtract(1, 'month');
      return { key, from: iso(m.startOf('month')), to: iso(m.endOf('month')) };
    }
    case 'this_fy':
      return { key, from: iso(fy), to: iso(fy.add(1, 'year').subtract(1, 'day')) };
    case 'last_fy':
      return { key, from: iso(fy.subtract(1, 'year')), to: iso(fy.subtract(1, 'day')) };
    case 'last_12_months':
      return { key, from: iso(t.subtract(11, 'month').startOf('month')), to: iso(t.endOf('month')) };
  }
}

/** Short month label for charts, e.g. "Apr 26". */
export function monthLabel(isoDate: string): string {
  return dayjs(isoDate).format('MMM YY');
}
