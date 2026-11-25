import { useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';

export const customerKeys = {
  all: (businessId: string) => ['business', businessId, 'customers'] as const,
  /** Every list page (not the details): invalidate after a delete, when the detail must not refetch. */
  lists: (businessId: string) => ['business', businessId, 'customers', 'list'] as const,
  list: (businessId: string, params: CustomerListParams) => ['business', businessId, 'customers', 'list', params] as const,
  detail: (businessId: string, id: string) => ['business', businessId, 'customers', 'detail', id] as const,
  /** A customer’s invoices and quotes live under the invoices and quotes keys, so changes there refresh them. */
  invoiceHistory: (businessId: string, id: string, page: number) => ['business', businessId, 'invoices', 'by-customer', id, page] as const,
  quoteHistory: (businessId: string, id: string, page: number) => ['business', businessId, 'quotes', 'by-customer', id, page] as const,
};

export interface CustomerListParams {
  search?: string;
  status?: 'active' | 'inactive' | 'all';
  sort?: 'name' | 'outstanding' | 'created';
  page?: number;
  per_page?: number;
}

/** Rows per page in a customer’s invoice and quote history. */
export const HISTORY_PAGE_SIZE = 10;

export function useCustomers(businessId: string, params: CustomerListParams) {
  return useQuery({
    queryKey: customerKeys.list(businessId, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/customers', { params: { path: { business_id: businessId }, query: params } })),
    placeholderData: (previous) => previous,
  });
}

export function useCustomer(businessId: string, id: string | null | undefined) {
  return useQuery({
    queryKey: customerKeys.detail(businessId, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/customers/{customer_id}', {
          params: { path: { business_id: businessId, customer_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

export function useCustomerInvoices(businessId: string, customerId: string, page: number) {
  return useQuery({
    queryKey: customerKeys.invoiceHistory(businessId, customerId, page),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/invoices', {
          params: { path: { business_id: businessId }, query: { customer_id: customerId, page, per_page: HISTORY_PAGE_SIZE } },
        }),
      ),
    placeholderData: (previous) => previous,
  });
}

export function useCustomerQuotes(businessId: string, customerId: string, page: number) {
  return useQuery({
    queryKey: customerKeys.quoteHistory(businessId, customerId, page),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/quotes', {
          params: { path: { business_id: businessId }, query: { customer_id: customerId, page, per_page: HISTORY_PAGE_SIZE } },
        }),
      ),
    placeholderData: (previous) => previous,
  });
}
