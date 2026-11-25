import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type Insights = components['schemas']['Insights'];
export type InsightSection = components['schemas']['InsightSection'];
export type Insight = components['schemas']['Insight'];
export type InsightNotice = components['schemas']['Notice'];
export type InsightFigure = components['schemas']['Figure'];
export type Tone = components['schemas']['Tone'];

export const insightKeys = {
  all: (businessId: string) => ['business', businessId, 'insights'] as const,
};

/** Insights and notices, worked out by the API from the records each time. */
export function useInsights() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: insightKeys.all(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/insights', { params: { path: { business_id: business.id } } })),
  });
}
