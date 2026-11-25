export const PAYMENT_METHODS = [
  { value: 'bank_transfer', label: 'Bank transfer' },
  { value: 'cash', label: 'Cash' },
  { value: 'card', label: 'Card' },
  { value: 'cheque', label: 'Cheque' },
  { value: 'online', label: 'Online payment' },
  { value: 'other', label: 'Other' },
  { value: 'write_off', label: 'Written off (bad debt)' },
];

export function paymentMethodLabel(value: string): string {
  return PAYMENT_METHODS.find((m) => m.value === value)?.label ?? value;
}

export function taxLabel(treatment: string, rate: string): string {
  switch (treatment) {
    case 'standard':
      return `${Number((Number(rate) * 100).toFixed(2))}%`;
    case 'zero_rated':
      return '0%';
    case 'exempt':
      return 'Exempt';
    default:
      return '—';
  }
}

export const FREQUENCIES = [
  { value: 'weekly', label: 'Every week' },
  { value: 'fortnightly', label: 'Every two weeks' },
  { value: 'monthly', label: 'Every month' },
  { value: 'quarterly', label: 'Every three months' },
  { value: 'yearly', label: 'Every year' },
] as const;

export const frequencyLabel = (value: string) => FREQUENCIES.find((f) => f.value === value)?.label ?? value;

/** The same day next month, for the first repeat of an invoice (YYYY-MM-DD; the 31st becomes the month's last day). */
export function nextMonth(date: string): string {
  const [y, m, d] = date.split('-').map(Number) as [number, number, number];
  const last = new Date(Date.UTC(y, m + 1, 0)).getUTCDate();
  const next = new Date(Date.UTC(y, m, Math.min(d, last)));
  return next.toISOString().slice(0, 10);
}
