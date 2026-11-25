import { SimpleGrid, Stack, Text } from '@mantine/core';
import { MoneyText } from '@/components/Money';
import type { ExpenseSplit } from './expenseMath';

/**
 * Business cost, claimable GST and private share of an expense, side by side. Paid from the business,
 * the private share is a drawing; paid personally, it never went through the business.
 */
export function SplitSummary({
  split,
  currency,
  paidPersonally = false,
}: {
  split: ExpenseSplit;
  currency: string;
  paidPersonally?: boolean;
}) {
  const figures: [string, number][] = [
    ['Business cost', split.expense_amount],
    ['GST to claim', split.gst_claim],
    [paidPersonally ? 'Private (not recorded)' : 'Private (drawings)', split.private_amount],
  ];
  return (
    <SimpleGrid cols={3} spacing="xs">
      {figures.map(([label, value]) => (
        <Stack key={label} gap={2}>
          <Text size="xs" c="dimmed">
            {label}
          </Text>
          <MoneyText value={value} currency={currency} fw={600} size="sm" c={value === 0 ? 'dimmed' : undefined} />
        </Stack>
      ))}
    </SimpleGrid>
  );
}
