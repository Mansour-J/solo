import type { components } from '@solo/api-client';
import { keepPreviousData, queryOptions, useQuery, type QueryKey } from '@tanstack/react-query';
import { businessKeys, useCurrentBusiness } from '@/features/businesses/api';
import { api, fetchBlob, unwrap } from '@/services/api/client';

export type Expense = components['schemas']['Expense'];
export type ExpenseDetail = components['schemas']['ExpenseDetail'];
export type ExpenseInput = components['schemas']['ExpenseInput'];
export type ExpensePage = components['schemas']['ExpensePage'];
export type ExpenseCategory = components['schemas']['CategoryRow'];
export type ExpenseAttachment = components['schemas']['AttachmentRow'];
export type JournalEntry = components['schemas']['JournalEntryView'];
export type PaymentMethod = components['schemas']['ExpensePaymentMethod'];

export interface ExpenseListParams {
  from?: string;
  to?: string;
  category_id?: string;
  search?: string;
  min_amount?: number;
  max_amount?: number;
  gst_treatment?: string;
  has_receipt?: boolean;
  classification?: string;
  include_deleted?: boolean;
  page?: number;
  per_page?: number;
}

export const expenseKeys = {
  /** Every expense list and detail: invalidate after any change to expenses. */
  all: (businessId: string) => ['business', businessId, 'expenses'] as const,
  list: (businessId: string, params: ExpenseListParams) => [...expenseKeys.all(businessId), 'list', params] as const,
  detail: (businessId: string, id: string) => [...expenseKeys.all(businessId), 'detail', id] as const,
  categories: (businessId: string) => ['business', businessId, 'expense-categories'] as const,
  /** Receipt files never change, so they live outside `all` and aren't refetched after edits. */
  attachment: (businessId: string, attachmentId: string) => ['business', businessId, 'expense-attachments', attachmentId] as const,
};

/** Queries to refresh after a change to expenses (and categories, whose counts follow expenses). */
export function expenseChangeKeys(businessId: string, { categories = true }: { categories?: boolean } = {}): QueryKey[] {
  return [
    expenseKeys.all(businessId),
    ...(categories ? [expenseKeys.categories(businessId)] : []),
    businessKeys.dashboard(businessId),
    businessKeys.activityAll(businessId),
  ];
}

export function useExpenses(params: ExpenseListParams) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: expenseKeys.list(business.id, params),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/expenses', { params: { path: { business_id: business.id }, query: params } })),
    placeholderData: keepPreviousData,
  });
}

export function useExpense(id: string | null | undefined) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: expenseKeys.detail(business.id, id ?? 'none'),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/expenses/{expense_id}', {
          params: { path: { business_id: business.id, expense_id: id ?? '' } },
        }),
      ),
    enabled: !!id,
  });
}

export function useExpenseCategories() {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: expenseKeys.categories(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/expense-categories', { params: { path: { business_id: business.id } } })),
  });
}

/** A receipt file, fetched with authentication. Use with `useQuery` or `queryClient.fetchQuery`. */
export function attachmentQuery(businessId: string, expenseId: string, attachmentId: string) {
  return queryOptions({
    queryKey: expenseKeys.attachment(businessId, attachmentId),
    queryFn: () => fetchBlob(`/api/v1/businesses/${businessId}/expenses/${expenseId}/attachments/${attachmentId}`),
    staleTime: Infinity,
    gcTime: 60_000,
  });
}
