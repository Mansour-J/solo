import { useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';

export const incomeKeys = {
  all: (businessId: string) => ['business', businessId, 'income'] as const,
  list: (businessId: string, params: IncomeListParams) => ['business', businessId, 'income', 'list', params] as const,
};

export interface IncomeListParams {
  /** Inclusive dates, YYYY-MM-DD. */
  from?: string;
  to?: string;
  search?: string;
  page?: number;
  per_page?: number;
}

/** A page of income records plus the count and gross total of everything matching the filter. */
export function useIncomeList(businessId: string, params: IncomeListParams) {
  return useQuery({
    queryKey: incomeKeys.list(businessId, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/income', { params: { path: { business_id: businessId }, query: params } })),
    placeholderData: (previous) => previous,
  });
}
