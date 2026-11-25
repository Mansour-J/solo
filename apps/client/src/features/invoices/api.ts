import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { businessKeys } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';

export type Invoice = components['schemas']['Invoice'];
export type InvoiceSummary = components['schemas']['InvoiceSummary'];
export type Quote = components['schemas']['Quote'];
export type QuoteSummary = components['schemas']['QuoteSummary'];
export type LineInput = components['schemas']['LineInput'];
export type Calculation = components['schemas']['Calculation'];
export type TaxTreatment = components['schemas']['TaxTreatment'];

export interface InvoiceListParams {
  status?: string;
  customer_id?: string;
  from?: string;
  to?: string;
  search?: string;
  sort?: string;
  page?: number;
  per_page?: number;
}

export interface QuoteListParams {
  status?: string;
  customer_id?: string;
  search?: string;
  page?: number;
  per_page?: number;
}

export type RecurringInvoice = components['schemas']['RecurringRow'];
export type Frequency = components['schemas']['Frequency'];

export const invoiceKeys = {
  list: (businessId: string, params: InvoiceListParams) => [...businessKeys.invoices(businessId), 'list', params] as const,
  recurring: (businessId: string) => [...businessKeys.invoices(businessId), 'recurring'] as const,
  detail: (businessId: string, id: string) => [...businessKeys.invoices(businessId), 'detail', id] as const,
};

export const quoteKeys = {
  list: (businessId: string, params: QuoteListParams) => [...businessKeys.quotes(businessId), 'list', params] as const,
  detail: (businessId: string, id: string) => [...businessKeys.quotes(businessId), 'detail', id] as const,
};

export function useInvoices(businessId: string, params: InvoiceListParams) {
  return useQuery({
    queryKey: invoiceKeys.list(businessId, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/invoices', { params: { path: { business_id: businessId }, query: params } })),
    placeholderData: keepPreviousData,
  });
}

export function useInvoice(businessId: string, id: string | undefined) {
  return useQuery({
    queryKey: invoiceKeys.detail(businessId, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/invoices/{invoice_id}', {
          params: { path: { business_id: businessId, invoice_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

export function useQuotes(businessId: string, params: QuoteListParams) {
  return useQuery({
    queryKey: quoteKeys.list(businessId, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/quotes', { params: { path: { business_id: businessId }, query: params } })),
    placeholderData: keepPreviousData,
  });
}

export function useQuote(businessId: string, id: string | undefined) {
  return useQuery({
    queryKey: quoteKeys.detail(businessId, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/quotes/{quote_id}', {
          params: { path: { business_id: businessId, quote_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

/** Every query a financial change can affect (payments also open or reverse allocations). */
export function financialKeys(businessId: string) {
  return [
    businessKeys.invoices(businessId),
    businessKeys.quotes(businessId),
    businessKeys.dashboard(businessId),
    businessKeys.activityAll(businessId),
    ['business', businessId, 'customers'] as const,
    ['business', businessId, 'allocations'] as const,
    ['business', businessId, 'insights'] as const,
    ['business', businessId, 'gst'] as const,
  ];
}

/** Recurring invoice schedules, active first. */
export function useRecurringInvoices(businessId: string) {
  return useQuery({
    queryKey: invoiceKeys.recurring(businessId),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/recurring-invoices', { params: { path: { business_id: businessId } } })),
  });
}
