import { useDebouncedValue } from '@mantine/hooks';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';
import type { TripInput } from './api';

/**
 * The claim for the trip being entered, worked out by the API (debounced). `input` is null until
 * the form has enough to calculate; errors carry the field to correct (e.g. no rate on the date).
 * `tripId` is the trip being edited, so its vehicle and rate are accepted even if since archived.
 */
export function useTripCalculation(businessId: string, input: TripInput | null, tripId?: string) {
  const [debounced] = useDebouncedValue(input, 300);
  const query = useQuery({
    queryKey: ['business', businessId, 'mileage', 'calculate', tripId ?? null, debounced],
    queryFn: () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/mileage/calculate', {
          params: { path: { business_id: businessId }, query: tripId ? { trip_id: tripId } : {} },
          body: debounced as TripInput,
        }),
      ),
    enabled: debounced !== null,
    placeholderData: keepPreviousData,
    staleTime: 60_000,
    retry: false,
  });
  return {
    calculation: input && !query.isError ? query.data : undefined,
    error: input ? query.error : null,
    isFetching: query.isFetching,
  };
}
