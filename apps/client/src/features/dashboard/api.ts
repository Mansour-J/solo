import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { businessKeys } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type Dashboard = components['schemas']['Dashboard'];
export type Basis = components['schemas']['ReportingBasis'];

export function useDashboard(businessId: string, from: string, to: string, basis: Basis) {
  return useQuery({
    queryKey: [...businessKeys.dashboard(businessId), from, to, basis],
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/dashboard', {
          params: { path: { business_id: businessId }, query: { from, to, basis } },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** Change from the previous period as a fraction, or null when there's nothing to compare with. */
export function change(current: number, previous: number): number | null {
  if (previous === 0) return null;
  return (current - previous) / Math.abs(previous);
}

export type KeyDates = components['schemas']['KeyDates'];
export type KeyDate = components['schemas']['KeyDate'];

/** Upcoming GST returns, tax payments and year end, with the days left (worked out by the API). */
export function useKeyDates(businessId: string) {
  return useQuery({
    queryKey: [...businessKeys.dashboard(businessId), 'key-dates'],
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/key-dates', {
          params: { path: { business_id: businessId } },
        }),
      ),
  });
}

/** "Today", "Tomorrow", "In 12 days", "3 days overdue". */
export function daysLeftLabel(days: number): string {
  if (days === 0) return 'Today';
  if (days === 1) return 'Tomorrow';
  if (days > 1) return `In ${days} days`;
  if (days === -1) return '1 day overdue';
  return `${-days} days overdue`;
}
