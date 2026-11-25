import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type GstReturns = components['schemas']['GstReturns'];
export type GstReturn = components['schemas']['GstReturn'];
export type GstReturnStatus = components['schemas']['GstReturnStatus'];
export type GstReturnBoxes = components['schemas']['GstReturnBoxes'];

export const gstKeys = {
  returns: (businessId: string) => ['business', businessId, 'gst', 'returns'] as const,
  boxes: (businessId: string, end: string) => ['business', businessId, 'gst', 'boxes', end] as const,
};

/** Every GST return period with its due date, what to pay and what was paid (from the API). */
export function useGstReturns() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: gstKeys.returns(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/gst/returns', { params: { path: { business_id: business.id } } })),
  });
}

/** The figures for IRD's return (boxes 5 to 15) for the period ending on `end`. */
export function useGstReturnBoxes(end: string | null) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: gstKeys.boxes(business.id, end ?? ''),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/gst/returns/boxes', {
          params: { path: { business_id: business.id }, query: { end: end ?? '' } },
        }),
      ),
    enabled: end !== null,
  });
}

const MONTH = new Intl.DateTimeFormat('en-NZ', { month: 'short', timeZone: 'UTC' });

/** "Apr–May 2026", "Oct 2026–Mar 2027" or "May 2026". */
export function periodLabel(start: string, end: string): string {
  const s = new Date(`${start}T00:00:00Z`);
  const e = new Date(`${end}T00:00:00Z`);
  const sameYear = s.getUTCFullYear() === e.getUTCFullYear();
  if (s.getUTCMonth() === e.getUTCMonth() && sameYear) return `${MONTH.format(e)} ${e.getUTCFullYear()}`;
  return sameYear
    ? `${MONTH.format(s)}–${MONTH.format(e)} ${e.getUTCFullYear()}`
    : `${MONTH.format(s)} ${s.getUTCFullYear()}–${MONTH.format(e)} ${e.getUTCFullYear()}`;
}

export const STATUS: Record<GstReturnStatus, { label: string; color: string }> = {
  in_progress: { label: 'In progress', color: 'gray' },
  to_file: { label: 'To file', color: 'brand' },
  overdue: { label: 'Overdue', color: 'red' },
  paid: { label: 'Paid', color: 'teal' },
  refund_due: { label: 'Refund due', color: 'yellow' },
  refunded: { label: 'Refunded', color: 'teal' },
  nil: { label: 'Nil return', color: 'gray' },
};

/** The months the business's return periods end in, e.g. "Jan, Mar, May, Jul, Sep and Nov". */
export function periodEndMonths(period: GstReturns['gst_period'], endMonth: number): string {
  const step = period === 'monthly' ? 1 : period === 'two_monthly' ? 2 : 6;
  if (step === 1) return 'every month';
  const months = Array.from({ length: 12 / step }, (_, i) => ((endMonth - 1 + i * step) % 12) + 1).sort((a, b) => a - b);
  const names = months.map((m) => MONTH.format(new Date(Date.UTC(2000, m - 1, 1))));
  return `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
}
