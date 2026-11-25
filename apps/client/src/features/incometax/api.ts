import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type IncomeTax = components['schemas']['IncomeTax'];
export type TaxYear = components['schemas']['TaxYear'];
export type TaxPayment = components['schemas']['TaxPayment'];

export const incomeTaxKeys = {
  summary: (businessId: string) => ['business', businessId, 'gst', 'income-tax'] as const,
};

/** Income tax, provisional tax and terminal tax for each income year (from the API). */
export function useIncomeTax() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: incomeTaxKeys.summary(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/income-tax', { params: { path: { business_id: business.id } } })),
  });
}

export const PAYMENT_STATUS: Record<TaxPayment['status'], { label: string; color: string }> = {
  in_progress: { label: 'Year in progress', color: 'gray' },
  upcoming: { label: 'Upcoming', color: 'brand' },
  overdue: { label: 'Overdue', color: 'red' },
  paid: { label: 'Paid', color: 'teal' },
  refund_due: { label: 'Refund due', color: 'yellow' },
  refunded: { label: 'Refunded', color: 'teal' },
};

/** "17.5%" from "0.175". */
export function percent(rate: string): string {
  return `${Number((Number(rate) * 100).toFixed(2))}%`;
}
