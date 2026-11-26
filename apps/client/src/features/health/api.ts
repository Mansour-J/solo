import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type BusinessHealth = components['schemas']['BusinessHealth'];
export type HealthCheck = components['schemas']['HealthCheck'];
export type HealthStatus = components['schemas']['HealthStatus'];
export type HealthArea = components['schemas']['HealthArea'];

export const healthKeys = {
  all: (businessId: string) => ['business', businessId, 'health'] as const,
};

/** The business health checks, worked out by the API from the business's settings and records each time. */
export function useBusinessHealth() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: healthKeys.all(business.id),
    // Recheck whenever the page opens: the fixes are made on other screens.
    staleTime: 0,
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/health', { params: { path: { business_id: business.id } } })),
  });
}
