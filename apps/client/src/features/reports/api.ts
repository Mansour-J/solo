import type { components } from '@solo/api-client';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, fetchBlob, unwrap } from '@/services/api/client';
import { saveBlob } from '@/services/platform';

export type Report = components['schemas']['Report'];
export type ReportSection = components['schemas']['Section'];
export type ReportColumn = components['schemas']['Column'];
export type ReportCell = components['schemas']['Cell'];
export type ReportKind =
  | 'profit_and_loss'
  | 'income_summary'
  | 'expense_summary'
  | 'statement_of_account'
  | 'receivables'
  | 'financial_statement'
  | 'mileage'
  | 'allocations'
  | 'gst'
  | 'balance_sheet'
  | 'equity_changes'
  | 'depreciation_schedule';
export type ExportFormat = 'pdf' | 'csv' | 'xlsx';

export interface ReportParams {
  from?: string;
  to?: string;
  basis?: 'cash' | 'accrual';
  customer_id?: string;
  category_id?: string;
  vehicle_id?: string;
  status?: 'pending' | 'confirmed' | 'skipped' | 'reversed' | 'cancelled';
}

export const reportKeys = {
  all: (businessId: string) => ['business', businessId, 'reports'] as const,
  report: (businessId: string, kind: ReportKind, params: ReportParams) => ['business', businessId, 'reports', kind, params] as const,
};

/** A report built by the API (every figure from the journal or the receivables). `enabled` false waits for a required filter. */
export function useReport(kind: ReportKind, params: ReportParams, enabled = true) {
  const business = useCurrentBusiness();
  return useQuery({
    queryKey: reportKeys.report(business.id, kind, params),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/reports/{kind}', {
          params: { path: { business_id: business.id, kind }, query: params },
        }),
      ),
    enabled,
    placeholderData: keepPreviousData,
    retry: false,
  });
}

const EXTENSIONS: Record<ExportFormat, string> = { pdf: 'pdf', csv: 'csv', xlsx: 'xlsx' };

/**
 * Downloads every report (except the per-customer statement) for one period and basis in one file: a
 * PDF with each report on new pages, an Excel workbook with a sheet each, or one CSV.
 */
export async function downloadAllReports(
  businessId: string,
  format: ExportFormat,
  params: Pick<ReportParams, 'from' | 'to' | 'basis'>,
  businessName: string,
) {
  const query = new URLSearchParams({ format });
  for (const [key, value] of Object.entries(params)) if (value) query.set(key, value);
  const blob = await fetchBlob(`/api/v1/businesses/${businessId}/reports/all/export?${query.toString()}`);
  const period = params.from && params.to ? ` ${params.from} to ${params.to}` : '';
  saveBlob(blob, `${businessName} - All reports${period}.${EXTENSIONS[format]}`.replace(/[/\\:*?"<>|]/g, ''));
}

/** Downloads the report as a file built by the API, named like the API names it. */
export async function downloadReport(businessId: string, kind: ReportKind, format: ExportFormat, params: ReportParams, report: Report) {
  const query = new URLSearchParams({ format });
  for (const [key, value] of Object.entries(params)) if (value) query.set(key, value);
  const blob = await fetchBlob(`/api/v1/businesses/${businessId}/reports/${kind}/export?${query.toString()}`);
  const period = report.period ? ` ${report.period.from} to ${report.period.to}` : ` ${report.generated_on}`;
  saveBlob(blob, `${report.title}${period}.${EXTENSIONS[format]}`.replace(/[/\\:*?"<>|]/g, ''));
}

/** Downloads the accountant pack (a zip) for the financial year ending `yearEnd`. */
export async function downloadAccountantPack(businessId: string, yearEnd: string, basis: string | undefined, businessName: string) {
  const query = new URLSearchParams({ year_end: yearEnd });
  if (basis) query.set('basis', basis);
  const blob = await fetchBlob(`/api/v1/businesses/${businessId}/reports/accountant-pack?${query.toString()}`);
  saveBlob(blob, `${businessName} accountant pack ${yearEnd}.zip`.replace(/[/\\:*?"<>|]/g, ''));
}

/** Downloads issued invoices (`sales`) or expenses (`bills`) for the period as a CSV Xero imports. */
export async function downloadXero(businessId: string, kind: 'sales' | 'bills', from: string, to: string) {
  const query = new URLSearchParams({ kind, from, to });
  const blob = await fetchBlob(`/api/v1/businesses/${businessId}/reports/xero-export?${query.toString()}`);
  saveBlob(blob, `Xero ${kind === 'sales' ? 'sales invoices' : 'bills'} ${from} to ${to}.csv`);
}
