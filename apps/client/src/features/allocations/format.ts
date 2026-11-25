// Labels and descriptions for allocations (display only; every amount comes from the API).
import { formatDate, formatMoney } from '@/lib/format';
import type { AllocationStatus, DestinationKind, PlanLine, Rule } from './api';

export const KIND_OPTIONS: { value: DestinationKind; label: string }[] = [
  { value: 'income_tax', label: 'Income tax' },
  { value: 'gst', label: 'GST' },
  { value: 'acc', label: 'ACC levies' },
  { value: 'savings', label: 'Savings' },
  { value: 'emergency_fund', label: 'Emergency fund' },
  { value: 'kiwisaver', label: 'KiwiSaver' },
  { value: 'investment', label: 'Investment' },
  { value: 'mortgage', label: 'Mortgage' },
  { value: 'personal_spending', label: 'Personal spending' },
  { value: 'custom', label: 'Other' },
];

export const STATUS_LABELS: Record<AllocationStatus, { label: string; color: string }> = {
  pending: { label: 'To review', color: 'yellow' },
  confirmed: { label: 'Allocated', color: 'teal' },
  skipped: { label: 'Skipped', color: 'gray' },
  reversed: { label: 'Reversed', color: 'gray' },
  cancelled: { label: 'Cancelled', color: 'gray' },
};

export function statusOf(status: string): { label: string; color: string } {
  return STATUS_LABELS[status as AllocationStatus] ?? { label: status, color: 'gray' };
}

const LIMIT_LABELS: Record<NonNullable<PlanLine['limited_by']>, string> = {
  maximum: 'Limited to the rule’s maximum',
  cap: 'Limited by the cap for the period',
  target: 'Limited by the destination’s target',
  remaining: 'Limited to what was left',
};

export function limitLabel(limit: PlanLine['limited_by']): string | null {
  return limit ? LIMIT_LABELS[limit] : null;
}

/** "20" from "20.00", "12.5" from "12.50". */
export function formatPercentage(value: string | null | undefined): string {
  if (!value) return '';
  return value.includes('.') ? value.replace(/\.?0+$/, '') : value;
}

const PERIOD_WORDS: Record<string, string> = { week: 'a week', fortnight: 'a fortnight', month: 'a month' };

/** What the rule sets aside: "20% of the amount received", "$50.00 fixed". */
export function ruleAmountText(rule: Rule, currency: string): string {
  if (rule.method === 'percentage') {
    return `${formatPercentage(rule.percentage)}% of ${rule.basis === 'remaining' ? 'what remains' : 'the amount received'}`;
  }
  return `${formatMoney(rule.fixed_amount ?? 0, currency)} from each receipt`;
}

/** The rule's conditions and limits, as short phrases. */
export function ruleDetails(rule: Rule, currency: string): string[] {
  const details: string[] = [];
  if (rule.source === 'invoice_payment') details.push('Invoice payments only');
  if (rule.source === 'income') {
    details.push(rule.income_type === 'sales' ? 'Sales income only' : rule.income_type === 'other' ? 'Other income only' : 'Income only');
  }
  if (rule.customer_name) details.push(`From ${rule.customer_name}`);
  if (rule.active_from && rule.active_to) details.push(`${formatDate(rule.active_from)} to ${formatDate(rule.active_to)}`);
  else if (rule.active_from) details.push(`From ${formatDate(rule.active_from)}`);
  else if (rule.active_to) details.push(`Until ${formatDate(rule.active_to)}`);
  if (rule.max_amount) details.push(`At most ${formatMoney(rule.max_amount, currency)} per receipt`);
  if (rule.cap_amount && rule.cap_period) {
    const period = rule.cap_period === 'custom' ? `every ${rule.cap_period_days ?? 1} days` : PERIOD_WORDS[rule.cap_period];
    details.push(`At most ${formatMoney(rule.cap_amount, currency)} ${period}`);
  }
  if (rule.stop_at_target) details.push('Stops at the target');
  return details;
}

export function sourceText(sourceType: string): string {
  return sourceType === 'invoice_payment' ? 'Invoice payment' : 'Income';
}
