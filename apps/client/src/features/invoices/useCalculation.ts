import type { components } from '@solo/api-client';
import { useDebouncedValue } from '@mantine/hooks';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';
import type { LineInput } from './api';
import type { LineValues } from './documentForm';

type CalculatedLine = components['schemas']['CalculatedLine'];

const decimal = /^\d+(\.\d{1,4})?$/;

function complete(line: LineValues): LineInput | null {
  if (line.unit_price === null || !decimal.test(line.quantity.trim()) || Number(line.quantity) <= 0) return null;
  const discount = line.discount_percent.trim() || '0';
  if (!/^\d{1,3}(\.\d{1,2})?$/.test(discount) || Number(discount) > 100) return null;
  return {
    description: line.description.trim() || 'Item',
    quantity: line.quantity.trim(),
    unit_price: line.unit_price,
    discount_percent: discount,
    tax_treatment: line.tax_treatment,
  };
}

/**
 * Live totals from the API while editing (debounced). Incomplete lines are left out; results are
 * mapped back to each line's key so rows show their own amount.
 */
export function useCalculation(businessId: string, issueDate: string, pricesIncludeGst: boolean, lines: LineValues[]) {
  const prepared = lines.map((l) => ({ key: l.key, input: complete(l) }));
  const inputs = prepared.flatMap((p) => (p.input ? [p.input] : []));
  const [debounced] = useDebouncedValue({ issueDate, pricesIncludeGst, inputs }, 300);
  const query = useQuery({
    queryKey: ['business', businessId, 'calculate', debounced],
    queryFn: () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/invoices/calculate', {
          params: { path: { business_id: businessId } },
          body: { issue_date: debounced.issueDate, prices_include_gst: debounced.pricesIncludeGst, lines: debounced.inputs },
        }),
      ),
    enabled: debounced.inputs.length > 0 && !!debounced.issueDate,
    placeholderData: keepPreviousData,
    staleTime: 60_000,
    retry: false,
  });
  const byKey = new Map<string, CalculatedLine>();
  if (query.data && query.data.lines.length === inputs.length) {
    let i = 0;
    for (const p of prepared) {
      if (p.input) {
        const line = query.data.lines[i];
        if (line) byKey.set(p.key, line);
        i += 1;
      }
    }
  }
  return { calculation: inputs.length > 0 ? query.data : undefined, byKey, isFetching: query.isFetching, error: query.error };
}
