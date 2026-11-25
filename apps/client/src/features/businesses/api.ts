import type { Business, BusinessSummary } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';
import { useSessionStore } from '@/stores/session';

export const businessKeys = {
  all: ['businesses'] as const,
  detail: (id: string) => ['business', id] as const,
  settings: (id: string) => ['business', id, 'settings'] as const,
  activity: (id: string, page: number) => ['business', id, 'activity', page] as const,
  /** Every page of the activity log: invalidate after any change to the business. */
  activityAll: (id: string) => ['business', id, 'activity'] as const,
  /** Dashboard figures: invalidate after any financial change (invoices, payments, expenses, income). */
  dashboard: (id: string) => ['business', id, 'dashboard'] as const,
  /** Invoices and quotes lists and details. */
  invoices: (id: string) => ['business', id, 'invoices'] as const,
  quotes: (id: string) => ['business', id, 'quotes'] as const,
};

export function useBusinesses() {
  return useQuery({ queryKey: businessKeys.all, queryFn: () => unwrap(api.GET('/api/v1/businesses')) });
}

export function useBusiness(id: string | null | undefined) {
  return useQuery({
    queryKey: businessKeys.detail(id ?? 'none'),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}', { params: { path: { business_id: id ?? '' } } })),
    enabled: !!id,
  });
}

export function useBusinessSettings(id: string) {
  return useQuery({
    queryKey: businessKeys.settings(id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/settings', { params: { path: { business_id: id } } })),
  });
}

export function useBusinessActivity(id: string, page: number) {
  return useQuery({
    queryKey: businessKeys.activity(id, page),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/activity', { params: { path: { business_id: id }, query: { page, per_page: 25 } } }),
      ),
    placeholderData: (previous) => previous,
  });
}

/** The selected business among the user's businesses (falls back to the first one). */
export function resolveCurrentBusiness(businesses: BusinessSummary[] | undefined, selectedId: string | null): BusinessSummary | null {
  if (!businesses?.length) return null;
  return businesses.find((b) => b.id === selectedId) ?? businesses[0] ?? null;
}

/** The business the user is working in. Only use below `RequireBusiness`, where it is loaded. */
export function useCurrentBusiness(): Business {
  const selectedId = useSessionStore((s) => s.currentBusinessId);
  const { data: businesses } = useBusinesses();
  const current = resolveCurrentBusiness(businesses, selectedId);
  const { data } = useBusiness(current?.id);
  if (!data) throw new Error('useCurrentBusiness used outside RequireBusiness');
  return data;
}
