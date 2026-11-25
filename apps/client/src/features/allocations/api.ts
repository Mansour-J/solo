import { useDebouncedValue } from '@mantine/hooks';
import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery, type QueryKey } from '@tanstack/react-query';
import { businessKeys, useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type Destination = components['schemas']['DestinationRow'];
export type DestinationInput = components['schemas']['DestinationInput'];
export type DestinationKind = components['schemas']['DestinationKind'];
export type Rule = components['schemas']['RuleRow'];
export type RuleInput = components['schemas']['RuleInput'];
export type Allocation = components['schemas']['AllocationDetail'];
export type AllocationPage = components['schemas']['AllocationPage'];
export type AllocationEntry = components['schemas']['EntryRow'];
export type PlanView = components['schemas']['PlanView'];
export type PlanLine = components['schemas']['PlanLine'];
export type LineInput = components['schemas']['ReviewLine'];
export type Balances = components['schemas']['Balances'];
export type DestinationBalance = components['schemas']['DestinationBalance'];
export type Spending = components['schemas']['SpendingRow'];
export type SpendingInput = components['schemas']['SpendingInput'];
export type AllocationStatus = 'pending' | 'confirmed' | 'skipped' | 'reversed' | 'cancelled';

export interface AllocationListParams {
  status?: AllocationStatus;
  destination_id?: string;
  from?: string;
  to?: string;
  invoice_payment_id?: string;
  income_id?: string;
  page?: number;
  per_page?: number;
}

export const allocationKeys = {
  /** Everything allocations: invalidate after any change to destinations, rules or allocations. */
  all: (businessId: string) => ['business', businessId, 'allocations'] as const,
  destinations: (businessId: string) => ['business', businessId, 'allocations', 'destinations'] as const,
  rules: (businessId: string) => ['business', businessId, 'allocations', 'rules'] as const,
  balances: (businessId: string) => ['business', businessId, 'allocations', 'balances'] as const,
  list: (businessId: string, params: AllocationListParams) => ['business', businessId, 'allocations', 'list', params] as const,
  detail: (businessId: string, id: string) => ['business', businessId, 'allocations', 'detail', id] as const,
  spending: (businessId: string) => ['business', businessId, 'allocations', 'spending'] as const,
};

/** Queries to refresh after a change to allocations (the dashboard shows balances, the activity log the change). */
export function allocationChangeKeys(businessId: string): QueryKey[] {
  return [allocationKeys.all(businessId), businessKeys.dashboard(businessId), businessKeys.activityAll(businessId)];
}

export function useDestinations() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.destinations(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/allocations/destinations', { params: { path: { business_id: business.id } } })),
  });
}

export function useRules() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.rules(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/allocations/rules', { params: { path: { business_id: business.id } } })),
  });
}

export function useAllocationBalances() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.balances(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/allocations/balances', { params: { path: { business_id: business.id } } })),
  });
}

export function useAllocations(params: AllocationListParams) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.list(business.id, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/allocations', { params: { path: { business_id: business.id }, query: params } })),
    placeholderData: keepPreviousData,
  });
}

export function useAllocation(id: string | null) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.detail(business.id, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/allocations/{allocation_id}', {
          params: { path: { business_id: business.id, allocation_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

/**
 * Totals for the amounts being reviewed, worked out by the API (debounced): what's allocated, what
 * remains and each destination's balance afterwards. Errors carry the field to correct.
 */
export function useReviewCalculation(allocationId: string, lines: LineInput[] | null) {
  const business = useCurrentBusiness();
  const [debounced] = useDebouncedValue(lines, 300);
  const query = useQuery({
    queryKey: ['business', business.id, 'allocations', 'calculate', allocationId, debounced],
    queryFn: () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/{allocation_id}/calculate', {
          params: { path: { business_id: business.id, allocation_id: allocationId } },
          body: { lines: debounced ?? [] },
        }),
      ),
    enabled: debounced !== null,
    placeholderData: keepPreviousData,
    retry: false,
  });
  return { calculation: lines && !query.isError ? query.data : undefined, error: lines ? query.error : null };
}

/** The allocation waiting for review for a receipt just recorded, if any (null when there's none). */
export async function findPendingAllocation(
  businessId: string,
  source: { invoice_payment_id: string } | { income_id: string },
): Promise<Allocation | null> {
  const page = await unwrap(
    api.GET('/api/v1/businesses/{business_id}/allocations', {
      params: { path: { business_id: businessId }, query: { ...source, status: 'pending', per_page: 1 } },
    }),
  );
  return page.items[0] ?? null;
}

/** Money used from what was set aside, newest first. */
export function useSpending() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: allocationKeys.spending(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/allocations/spending', { params: { path: { business_id: business.id } } })),
  });
}
