import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery, type QueryKey } from '@tanstack/react-query';
import { businessKeys, useCurrentBusiness } from '@/features/businesses/api';
import { api, fetchBlob, unwrap } from '@/services/api/client';
import { saveBlob } from '@/services/platform';

export type Vehicle = components['schemas']['VehicleRow'];
export type VehicleInput = components['schemas']['VehicleInput'];
export type VehicleType = components['schemas']['VehicleType'];
export type FuelType = components['schemas']['FuelType'];
export type MileageRate = components['schemas']['MileageRate'];
export type RateVersion = components['schemas']['RateVersionRow'];
export type Trip = components['schemas']['Trip'];
export type TripDetail = components['schemas']['TripDetail'];
export type TripInput = components['schemas']['TripInput'];
export type TripPage = components['schemas']['TripPage'];
export type TripCalculation = components['schemas']['TripCalculation'];
export type FrequentTrip = components['schemas']['FrequentTrip'];
export type TripTotals = components['schemas']['TripTotals'];
export type MileageSummary = components['schemas']['MileageSummary'];

export interface TripListParams {
  /** Inclusive dates, YYYY-MM-DD. */
  from?: string;
  to?: string;
  vehicle_id?: string;
  search?: string;
  page?: number;
  per_page?: number;
}

export interface MileagePeriod {
  from: string;
  to: string;
  vehicle_id?: string;
}

export const mileageKeys = {
  /** Everything mileage: invalidate after any change to vehicles, rates or trips. */
  all: (businessId: string) => ['business', businessId, 'mileage'] as const,
  vehicles: (businessId: string) => ['business', businessId, 'mileage', 'vehicles'] as const,
  rates: (businessId: string) => ['business', businessId, 'mileage', 'rates'] as const,
  trips: (businessId: string, params: TripListParams) => ['business', businessId, 'mileage', 'trips', params] as const,
  trip: (businessId: string, id: string) => ['business', businessId, 'mileage', 'trip', id] as const,
  summary: (businessId: string, period: MileagePeriod) => ['business', businessId, 'mileage', 'summary', period] as const,
};

/** Queries to refresh after a change to mileage: trips post claims, so the dashboard and activity change too. */
export function mileageChangeKeys(businessId: string): QueryKey[] {
  return [mileageKeys.all(businessId), businessKeys.dashboard(businessId), businessKeys.activityAll(businessId)];
}

/** Routes recorded at least twice in the last year, most often first. */
export function useFrequentTrips() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: [...mileageKeys.all(business.id), 'frequent-trips'],
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/mileage/frequent-trips', { params: { path: { business_id: business.id } } })),
  });
}

export function useVehicles() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: mileageKeys.vehicles(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/mileage/vehicles', { params: { path: { business_id: business.id } } })),
  });
}

export function useMileageRates() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: mileageKeys.rates(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/mileage/rates', { params: { path: { business_id: business.id } } })),
  });
}

export function useTrips(params: TripListParams) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: mileageKeys.trips(business.id, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/mileage/trips', { params: { path: { business_id: business.id }, query: params } })),
    placeholderData: keepPreviousData,
  });
}

export function useTrip(id: string | null | undefined) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: mileageKeys.trip(business.id, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/mileage/trips/{trip_id}', {
          params: { path: { business_id: business.id, trip_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

export function useMileageSummary(period: MileagePeriod) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: mileageKeys.summary(business.id, period),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/mileage/summary', { params: { path: { business_id: business.id }, query: period } }),
      ),
    placeholderData: keepPreviousData,
  });
}

/** Downloads the trips of a period as CSV (the API builds the file). */
export async function downloadMileageCsv(businessId: string, period: MileagePeriod): Promise<void> {
  const query = new URLSearchParams({ from: period.from, to: period.to, ...(period.vehicle_id ? { vehicle_id: period.vehicle_id } : {}) });
  const blob = await fetchBlob(`/api/v1/businesses/${businessId}/mileage/export?${query.toString()}`);
  saveBlob(blob, `Mileage ${period.from} to ${period.to}.csv`);
}
