// The rule form: values, validation (mirroring the API) and conversion to the API shape.
import { z } from 'zod';
import { formatPercentage } from './format';
import type { Rule, RuleInput } from './api';

export type CapPeriod = 'week' | 'fortnight' | 'month' | 'custom';

export interface RuleFormValues {
  destination_id: string | null;
  method: 'percentage' | 'fixed';
  /** As typed, e.g. "20" or "12.5". */
  percentage: string;
  basis: 'received' | 'remaining';
  /** Minor units. */
  fixed_amount: number | null;
  source: 'any' | 'invoice_payment' | 'income';
  /** '' for all income. */
  income_type: '' | 'sales' | 'other';
  customer_id: string | null;
  active_from: string | null;
  active_to: string | null;
  max_amount: number | null;
  /** Whether the rule has a cap per period. */
  capped: boolean;
  cap_amount: number | null;
  cap_period: CapPeriod;
  /** NumberInput reports '' when cleared. */
  cap_period_days: number | string;
  cap_anchor: string | null;
  stop_at_target: boolean;
  notes: string;
}

export function emptyRuleValues(destinationId: string | null, today: string): RuleFormValues {
  return {
    destination_id: destinationId,
    method: 'percentage',
    percentage: '',
    basis: 'received',
    fixed_amount: null,
    source: 'any',
    income_type: '',
    customer_id: null,
    active_from: null,
    active_to: null,
    max_amount: null,
    capped: false,
    cap_amount: null,
    cap_period: 'month',
    cap_period_days: 30,
    cap_anchor: today,
    stop_at_target: false,
    notes: '',
  };
}

export function fromRule(rule: Rule, today: string): RuleFormValues {
  return {
    destination_id: rule.destination_id,
    method: rule.method === 'fixed' ? 'fixed' : 'percentage',
    percentage: formatPercentage(rule.percentage),
    basis: rule.basis === 'remaining' ? 'remaining' : 'received',
    fixed_amount: rule.fixed_amount ?? null,
    source: (rule.source as RuleFormValues['source']) ?? 'any',
    income_type: (rule.income_type as RuleFormValues['income_type']) ?? '',
    customer_id: rule.customer_id ?? null,
    active_from: rule.active_from ?? null,
    active_to: rule.active_to ?? null,
    max_amount: rule.max_amount ?? null,
    capped: rule.cap_amount != null,
    cap_amount: rule.cap_amount ?? null,
    cap_period: (rule.cap_period as CapPeriod | null) ?? 'month',
    cap_period_days: rule.cap_period_days ?? 30,
    cap_anchor: rule.cap_anchor ?? today,
    stop_at_target: rule.stop_at_target,
    notes: rule.notes,
  };
}

const PERCENT = /^\d{1,3}(\.\d{1,2})?$/;

export function validPercentage(value: string): boolean {
  const text = value.trim();
  return PERCENT.test(text) && Number(text) > 0 && Number(text) <= 100;
}

const positive = (v: number | null) => v !== null && v > 0;

export const ruleSchema = z
  .object({
    destination_id: z
      .string()
      .nullable()
      .refine((v) => !!v, 'Choose a destination.'),
    notes: z.string().max(1000, 'Use 1,000 characters or fewer.'),
  })
  .loose()
  .superRefine((raw, ctx) => {
    const v = raw as unknown as RuleFormValues;
    if (v.method === 'percentage' && !validPercentage(v.percentage)) {
      ctx.addIssue({ code: 'custom', path: ['percentage'], message: 'Enter a percentage greater than 0 and up to 100.' });
    }
    if (v.method === 'fixed' && !positive(v.fixed_amount)) {
      ctx.addIssue({ code: 'custom', path: ['fixed_amount'], message: 'Enter an amount.' });
    }
    if (v.active_from && v.active_to && v.active_to < v.active_from) {
      ctx.addIssue({ code: 'custom', path: ['active_to'], message: 'The end date can’t be before the start date.' });
    }
    if (v.max_amount !== null && !positive(v.max_amount)) {
      ctx.addIssue({ code: 'custom', path: ['max_amount'], message: 'Enter an amount greater than zero, or leave it empty.' });
    }
    if (v.capped) {
      if (!positive(v.cap_amount)) ctx.addIssue({ code: 'custom', path: ['cap_amount'], message: 'Enter the cap for each period.' });
      if ((v.cap_period === 'fortnight' || v.cap_period === 'custom') && !v.cap_anchor) {
        ctx.addIssue({ code: 'custom', path: ['cap_anchor'], message: 'Enter the date the first period starts.' });
      }
      const days = Number(v.cap_period_days);
      if (v.cap_period === 'custom' && (!Number.isInteger(days) || days < 1 || days > 366)) {
        ctx.addIssue({ code: 'custom', path: ['cap_period_days'], message: 'Enter a period of 1 to 366 days.' });
      }
    }
  });

/** The request body. Only the fields for the chosen method, source and cap are sent. */
export function toRuleInput(values: RuleFormValues): RuleInput {
  const percentage = values.method === 'percentage';
  const anchored = values.capped && (values.cap_period === 'fortnight' || values.cap_period === 'custom');
  return {
    destination_id: values.destination_id ?? '',
    method: values.method,
    percentage: percentage ? values.percentage.trim() : null,
    basis: values.basis,
    fixed_amount: percentage ? null : values.fixed_amount,
    source: values.source,
    income_type: values.source === 'income' && values.income_type ? values.income_type : null,
    customer_id: values.customer_id,
    active_from: values.active_from,
    active_to: values.active_to,
    max_amount: values.max_amount,
    cap_amount: values.capped ? values.cap_amount : null,
    cap_period: values.capped ? values.cap_period : null,
    cap_period_days: values.capped && values.cap_period === 'custom' ? Number(values.cap_period_days) : null,
    cap_anchor: anchored ? values.cap_anchor : null,
    stop_at_target: values.stop_at_target,
    notes: values.notes.trim(),
  };
}
