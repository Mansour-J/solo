import { Group, Select } from '@mantine/core';
import { DatePickerInput } from '@mantine/dates';
import { CalendarRange } from 'lucide-react';
import { dateInputFormat } from '@/lib/format';
import { PERIOD_LABELS, periodRange, type Period, type PeriodKey } from '@/lib/periods';

/** Preset periods (this month, financial years…) plus a custom range. */
export function PeriodPicker({
  value,
  onChange,
  today,
  fyStart,
}: {
  value: Period;
  onChange: (period: Period) => void;
  today: string;
  fyStart: string;
}) {
  return (
    <Group gap="xs" wrap="wrap">
      <Select
        aria-label="Period"
        leftSection={<CalendarRange size={15} />}
        w={210}
        value={value.key}
        data={(Object.keys(PERIOD_LABELS) as PeriodKey[]).map((key) => ({ value: key, label: PERIOD_LABELS[key] }))}
        onChange={(key) => {
          if (!key) return;
          if (key === 'custom') onChange({ ...value, key: 'custom' });
          else onChange(periodRange(key as Exclude<PeriodKey, 'custom'>, today, fyStart));
        }}
      />
      {value.key === 'custom' && (
        <DatePickerInput
          type="range"
          aria-label="Custom date range"
          valueFormat={dateInputFormat()}
          w={250}
          value={[value.from, value.to]}
          onChange={([from, to]) => {
            if (from && to) onChange({ key: 'custom', from, to });
          }}
        />
      )}
    </Group>
  );
}
