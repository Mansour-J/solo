import { Badge, Text, TextInput, type TextInputProps, type TextProps } from '@mantine/core';
import { useState } from 'react';
import { formatMoney, minorToDecimalString, minorUnits, parseMoney } from '@/lib/format';
import { STATUS_COLORS } from '@/theme/theme';

/** An amount in minor units, formatted for the currency, with tabular figures. */
export function MoneyText({ value, currency = 'NZD', ...props }: { value: number; currency?: string } & TextProps) {
  return (
    <Text component="span" className="tabular" {...props}>
      {formatMoney(value, currency)}
    </Text>
  );
}

const STATUS_LABELS: Record<string, string> = {
  draft: 'Draft',
  issued: 'Issued',
  sent: 'Sent',
  partially_paid: 'Part paid',
  paid: 'Paid',
  written_off: 'Written off',
  overdue: 'Overdue',
  cancelled: 'Cancelled',
  accepted: 'Accepted',
  rejected: 'Declined',
  expired: 'Expired',
  converted: 'Invoiced',
};

/** Document status with a consistent colour and a text label (colour is never the only signal). */
export function StatusBadge({ status }: { status: string }) {
  const color = status === 'converted' ? 'teal' : (STATUS_COLORS[status as keyof typeof STATUS_COLORS] ?? 'gray');
  return (
    <Badge color={color} variant={status === 'overdue' ? 'filled' : 'light'} style={{ flexShrink: 0 }}>
      {STATUS_LABELS[status] ?? status}
    </Badge>
  );
}

export interface MoneyInputProps extends Omit<TextInputProps, 'value' | 'onChange'> {
  /** Minor units, or null when empty. */
  value: number | null;
  onChange: (value: number | null) => void;
  currency?: string;
  allowNegative?: boolean;
}

/**
 * Text input for money that keeps what the user types and reports minor units (parsed from the
 * string, never through floating point). Invalid input reports null and shows an error on blur.
 */
export function MoneyInput({ value, onChange, currency = 'NZD', allowNegative = false, error, ...props }: MoneyInputProps) {
  const [text, setText] = useState(value === null ? '' : minorToDecimalString(value, currency));
  const [touched, setTouched] = useState(false);
  const parsed = text.trim() === '' ? null : parseMoney(text, currency);
  // Keep the text in step when the value is changed from outside (e.g. form reset).
  const [lastValue, setLastValue] = useState(value);
  if (value !== lastValue) {
    setLastValue(value);
    if (value !== parsed) setText(value === null ? '' : minorToDecimalString(value, currency));
  }
  const invalid = text.trim() !== '' && (parsed === null || (!allowNegative && parsed < 0));
  const digits = minorUnits(currency);
  return (
    <TextInput
      inputMode="decimal"
      leftSection={
        <Text size="sm" c="dimmed">
          $
        </Text>
      }
      value={text}
      onChange={(event) => {
        const next = event.currentTarget.value;
        setText(next);
        const amount = next.trim() === '' ? null : parseMoney(next, currency);
        const valid = amount === null || allowNegative || amount >= 0;
        if (valid) {
          setLastValue(amount);
          onChange(amount);
        }
      }}
      onBlur={(event) => {
        setTouched(true);
        if (parsed !== null && !invalid) setText(minorToDecimalString(parsed, currency));
        props.onBlur?.(event);
      }}
      error={error ?? (touched && invalid ? `Enter an amount with up to ${digits} decimal places.` : undefined)}
      classNames={{ input: 'tabular' }}
      {...props}
    />
  );
}
