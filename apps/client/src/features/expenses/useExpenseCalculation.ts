import type { components } from '@solo/api-client';
import { useDebouncedValue } from '@mantine/hooks';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';

export type ExpenseCalculationInput = components['schemas']['ExpenseCalculationInput'];
export type ExpenseCalculation = components['schemas']['ExpenseCalculation'];

/**
 * How the API would book the expense being entered (GST, business and private shares), debounced.
 * `input` is null until there's an amount and a date. The figures are the API's: saving produces
 * exactly these.
 */
export function useExpenseCalculation(businessId: string, input: ExpenseCalculationInput | null) {
  const [debounced] = useDebouncedValue(input, 300);
  const query = useQuery({
    queryKey: ['business', businessId, 'expenses', 'calculate', debounced],
    queryFn: () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/expenses/calculate', {
          params: { path: { business_id: businessId } },
          body: debounced as ExpenseCalculationInput,
        }),
      ),
    enabled: debounced !== null,
    placeholderData: keepPreviousData,
    staleTime: 60_000,
    retry: false,
  });
  return { calculation: input && !query.isError ? query.data : undefined, isFetching: query.isFetching };
}
