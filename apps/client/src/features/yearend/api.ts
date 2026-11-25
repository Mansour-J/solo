import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { financialKeys } from '@/features/invoices/api';
import { api, unwrap } from '@/services/api/client';

export type Movement = components['schemas']['MovementRow'];
export type MovementKind = components['schemas']['MovementKind'];
export type MovementInput = components['schemas']['MovementInput'];
export type Asset = components['schemas']['AssetRow'];
export type AssetInput = components['schemas']['AssetInput'];
export type DepreciationYear = components['schemas']['DepreciationYear'];
export type OpeningView = components['schemas']['OpeningView'];
export type OpeningInput = components['schemas']['OpeningInput'];
export type HomeOfficeYear = components['schemas']['HomeOfficeYear'];
export type AssetUse = components['schemas']['AssetUse'];
export type Disposal = components['schemas']['DisposalRow'];

export const yearEndKeys = {
  all: (businessId: string) => ['business', businessId, 'year-end'] as const,
  movements: (businessId: string, page: number, kind: MovementKind | null) =>
    ['business', businessId, 'year-end', 'movements', page, kind] as const,
  assets: (businessId: string) => ['business', businessId, 'year-end', 'assets'] as const,
  depreciation: (businessId: string, yearEnd: string) => ['business', businessId, 'year-end', 'depreciation', yearEnd] as const,
  opening: (businessId: string) => ['business', businessId, 'year-end', 'opening'] as const,
  homeOffice: (businessId: string, yearEnd: string) => ['business', businessId, 'year-end', 'home-office', yearEnd] as const,
  assetUse: (businessId: string, assetId: string) => ['business', businessId, 'year-end', 'asset-use', assetId] as const,
  disposal: (businessId: string, assetId: string) => ['business', businessId, 'year-end', 'disposal', assetId] as const,
};

/** Everything a year-end change can move: these records, the reports and the usual financial views. */
export function yearEndChangeKeys(businessId: string) {
  return [yearEndKeys.all(businessId), ['business', businessId, 'reports'] as const, ...financialKeys(businessId)];
}

export const PER_PAGE = 25;

export function useMovements(page: number, kind: MovementKind | null) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.movements(business.id, page, kind),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/cash-movements', {
          params: { path: { business_id: business.id }, query: { page, per_page: PER_PAGE, ...(kind ? { kind } : {}) } },
        }),
      ),
    placeholderData: keepPreviousData,
  });
}

export function useAssets() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.assets(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/assets', { params: { path: { business_id: business.id } } })),
  });
}

export function useDepreciationYear(yearEnd: string) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.depreciation(business.id, yearEnd),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/assets/depreciation', {
          params: { path: { business_id: business.id }, query: { year_end: yearEnd } },
        }),
      ),
  });
}

export function useOpening() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.opening(business.id),
    queryFn: () => unwrap(api.GET('/api/v1/businesses/{business_id}/opening-balances', { params: { path: { business_id: business.id } } })),
  });
}

export function useHomeOffice(yearEnd: string) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.homeOffice(business.id, yearEnd),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/home-office', {
          params: { path: { business_id: business.id }, query: { year_end: yearEnd } },
        }),
      ),
  });
}

/** An asset's GST change-of-use periods and adjustments (IR375). */
export function useAssetUse(assetId: string) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.assetUse(business.id, assetId),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/assets/{asset_id}/use-reviews', {
          params: { path: { business_id: business.id, asset_id: assetId } },
        }),
      ),
  });
}

/** An asset's sale or scrapping, when it has one. */
export function useDisposal(assetId: string, enabled: boolean) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: yearEndKeys.disposal(business.id, assetId),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/assets/{asset_id}/disposal', {
          params: { path: { business_id: business.id, asset_id: assetId } },
        }),
      ),
    enabled,
  });
}
