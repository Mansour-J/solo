import { Badge, Box, Group, Progress, SegmentedControl, Skeleton, Stack, Table, Text } from '@mantine/core';
import { useState, type ReactNode } from 'react';
import { MoneyText } from '@/components/Money';
import { SectionCard } from '@/components/States';
import { formatMoney, formatPercent } from '@/lib/format';
import type { Balances, DestinationBalance } from './api';

function TargetNote({ d, currency }: { d: DestinationBalance; currency: string }) {
  if (d.target_amount == null) return null;
  return (
    <Text size="xs" c="dimmed">
      {formatPercent(Math.min(d.target_progress ?? 0, 999) / 100)} of {formatMoney(d.target_amount, currency)} target
    </Text>
  );
}

function MinimumBadge({ d }: { d: DestinationBalance }) {
  return d.below_minimum ? (
    <Badge size="xs" variant="light" color="orange">
      Below minimum
    </Badge>
  ) : null;
}

/**
 * Balances by destination: a ranked list with bars in one hue (the distribution), or the same data
 * as a table. Bars show each destination's share of everything set aside.
 */
export function BalancesPanel({
  balances,
  loading,
  title = 'Balances',
  actions,
  limit,
}: {
  balances?: Balances;
  loading: boolean;
  title?: string;
  actions?: ReactNode;
  /** Show only the largest few (dashboard). */
  limit?: number;
}) {
  const [view, setView] = useState<'chart' | 'table'>('chart');
  const currency = balances?.currency ?? 'NZD';
  const all = [...(balances?.destinations ?? [])].sort((a, b) => b.balance - a.balance);
  const rows = limit ? all.filter((d) => d.balance !== 0).slice(0, limit) : all;
  const total = balances?.total ?? 0;
  const positiveTotal = all.reduce((sum, d) => sum + Math.max(d.balance, 0), 0);
  // The chart shows what holds a balance; the table lists every destination.
  const charted = rows.filter((d) => d.balance !== 0);
  const unused = rows.filter((d) => d.balance === 0);
  return (
    <SectionCard
      title={title}
      description={balances ? `${formatMoney(total, currency)} set aside in total` : undefined}
      actions={
        <Group gap="xs" wrap="nowrap">
          <SegmentedControl
            size="xs"
            value={view}
            onChange={(v) => setView(v as 'chart' | 'table')}
            data={[
              { value: 'chart', label: 'Chart' },
              { value: 'table', label: 'Table' },
            ]}
            aria-label="Show as chart or table"
          />
          {actions}
        </Group>
      }
    >
      {loading && !balances ? (
        <Skeleton height={200} />
      ) : rows.length === 0 || positiveTotal === 0 ? (
        <Text size="sm" c="dimmed" py="lg" ta="center">
          Nothing set aside yet. Confirmed allocations add to these balances.
        </Text>
      ) : view === 'chart' ? (
        <Stack gap="sm">
          {charted.map((d) => (
            <div key={d.id}>
              <Group justify="space-between" gap="xs" mb={4} wrap="nowrap">
                <Group gap={6} wrap="nowrap" style={{ minWidth: 0 }}>
                  <Text size="sm" className="clamp-1">
                    {d.name}
                    {d.archived ? ' (archived)' : ''}
                  </Text>
                  <MinimumBadge d={d} />
                </Group>
                <Group gap={6} wrap="nowrap">
                  <MoneyText value={d.balance} currency={currency} size="sm" fw={500} />
                  <Text size="xs" c="dimmed" w={36} ta="right" className="tabular">
                    {formatPercent(positiveTotal ? Math.max(d.balance, 0) / positiveTotal : 0)}
                  </Text>
                </Group>
              </Group>
              <Progress
                value={positiveTotal ? (Math.max(d.balance, 0) / positiveTotal) * 100 : 0}
                color="var(--app-income)"
                size={6}
                radius="xl"
                aria-hidden
              />
              <TargetNote d={d} currency={currency} />
            </div>
          ))}
          {unused.length > 0 && (
            <Text size="xs" c="dimmed">
              Nothing set aside yet for {unused.map((d) => d.name).join(', ')}.
            </Text>
          )}
        </Stack>
      ) : (
        <Table.ScrollContainer minWidth={420}>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Destination</Table.Th>
                <Table.Th ta="right">Balance</Table.Th>
                <Table.Th ta="right">Share</Table.Th>
                <Table.Th ta="right">Target</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {rows.map((d) => (
                <Table.Tr key={d.id}>
                  <Table.Td>
                    <Box>
                      <Text size="sm">
                        {d.name}
                        {d.archived ? ' (archived)' : ''}
                      </Text>
                      <MinimumBadge d={d} />
                    </Box>
                  </Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={d.balance} currency={currency} size="sm" />
                  </Table.Td>
                  <Table.Td ta="right" className="tabular">
                    {formatPercent(positiveTotal ? Math.max(d.balance, 0) / positiveTotal : 0)}
                  </Table.Td>
                  <Table.Td ta="right">
                    {d.target_amount != null ? (
                      <Text size="sm" className="tabular">
                        {formatPercent((d.target_progress ?? 0) / 100)} of {formatMoney(d.target_amount, currency)}
                      </Text>
                    ) : (
                      <Text size="sm" c="dimmed">
                        —
                      </Text>
                    )}
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Table.ScrollContainer>
      )}
    </SectionCard>
  );
}
