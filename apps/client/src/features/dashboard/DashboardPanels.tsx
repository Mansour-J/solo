import {
  Anchor,
  Badge,
  Box,
  Group,
  Paper,
  Progress,
  SegmentedControl,
  Skeleton,
  Stack,
  Table,
  Text,
  ThemeIcon,
  Tooltip,
  UnstyledButton,
} from '@mantine/core';
import { BarChart } from '@mantine/charts';
import { ArrowDownRight, ArrowUpRight, Minus, type LucideIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { Link } from 'react-router';
import { MoneyText, StatusBadge } from '@/components/Money';
import { SectionCard } from '@/components/States';
import { formatKm } from '@/features/mileage/format';
import { formatDate, formatMoney, formatMoneyCompact, formatNumber, formatPercent } from '@/lib/format';
import { monthLabel } from '@/lib/periods';
import { change, type Dashboard } from './api';

/** A headline figure with an optional comparison to the previous period and a link to the detail. */
export function KpiCard({
  label,
  value,
  icon: Icon,
  previous,
  current,
  inverse,
  hint,
  to,
  loading,
}: {
  label: string;
  value: ReactNode;
  icon: LucideIcon;
  current?: number;
  previous?: number;
  /** Lower is better (expenses). */
  inverse?: boolean;
  hint?: ReactNode;
  to?: string;
  loading?: boolean;
}) {
  const delta = current !== undefined && previous !== undefined ? change(current, previous) : undefined;
  const good = delta !== undefined && delta !== null && (inverse ? delta < 0 : delta > 0);
  const bad = delta !== undefined && delta !== null && (inverse ? delta > 0 : delta < 0);
  const Trend = delta === null || delta === 0 ? Minus : (delta ?? 0) > 0 ? ArrowUpRight : ArrowDownRight;
  const body = (
    <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)" h="100%">
      <Stack gap={6}>
        <Group justify="space-between" wrap="nowrap">
          <Text size="xs" c="dimmed" fw={500}>
            {label}
          </Text>
          <ThemeIcon size={26} radius="md" variant="light" color="brand">
            <Icon size={15} />
          </ThemeIcon>
        </Group>
        {loading ? (
          <Skeleton height={30} width="60%" />
        ) : (
          <Text fz={24} fw={650} lh={1.15} className="tabular">
            {value}
          </Text>
        )}
        <Group gap={4} h={18} wrap="nowrap">
          {!loading && delta !== undefined && (
            <Tooltip label="Compared with the previous period of the same length">
              <Group gap={2} wrap="nowrap" c={good ? 'teal' : bad ? 'red' : 'dimmed'}>
                <Trend size={14} />
                <Text size="xs" fw={600} className="tabular">
                  {delta === null ? 'No earlier figures' : formatPercent(Math.abs(delta), 0)}
                </Text>
              </Group>
            </Tooltip>
          )}
          {hint && (
            <Text size="xs" c="dimmed" className="clamp-1">
              {hint}
            </Text>
          )}
        </Group>
      </Stack>
    </Paper>
  );
  return to ? (
    <UnstyledButton component={Link} to={to} h="100%" aria-label={`${label}: view details`}>
      {body}
    </UnstyledButton>
  ) : (
    body
  );
}

/** Income and expenses per month: grouped bars with a legend and tooltip, or the same data as a table. */
export function MonthlyChart({ data, loading }: { data?: Dashboard; loading: boolean }) {
  const [view, setView] = useState<'chart' | 'table'>('chart');
  const currency = data?.currency ?? 'NZD';
  const rows = (data?.monthly ?? []).map((m) => ({
    month: monthLabel(m.month),
    Income: m.income / 100,
    Expenses: m.expenses / 100,
    raw: m,
  }));
  const empty = rows.every((r) => r.raw.income === 0 && r.raw.expenses === 0);
  return (
    <SectionCard
      title="Income and expenses"
      description={data ? `By month · ${data.basis === 'cash' ? 'cash basis' : 'accrual basis'}` : undefined}
      actions={
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
      }
    >
      {loading && !data ? (
        <Skeleton height={260} />
      ) : empty ? (
        <Box h={260} style={{ display: 'grid', placeItems: 'center' }}>
          <Text size="sm" c="dimmed">
            No income or expenses in this period yet.
          </Text>
        </Box>
      ) : view === 'chart' ? (
        <BarChart
          h={260}
          data={rows}
          dataKey="month"
          series={[
            { name: 'Income', color: 'var(--app-income)' },
            { name: 'Expenses', color: 'var(--app-expense)' },
          ]}
          withLegend
          legendProps={{ verticalAlign: 'top', height: 32 }}
          gridAxis="y"
          tickLine="none"
          barProps={{ radius: [4, 4, 0, 0] }}
          barChartProps={{ barGap: 2 }}
          valueFormatter={(v) => formatMoney(Math.round(v * 100), currency)}
          yAxisProps={{ tickFormatter: (v: number) => formatMoneyCompact(Math.round(v * 100), currency), width: 64 }}
        />
      ) : (
        <Table.ScrollContainer minWidth={360}>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Month</Table.Th>
                <Table.Th ta="right">Income</Table.Th>
                <Table.Th ta="right">Expenses</Table.Th>
                <Table.Th ta="right">Difference</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {rows.map((r) => (
                <Table.Tr key={r.month}>
                  <Table.Td>{r.month}</Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={r.raw.income} currency={currency} size="sm" />
                  </Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={r.raw.expenses} currency={currency} size="sm" />
                  </Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={r.raw.income - r.raw.expenses} currency={currency} size="sm" />
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

/** Spending by category as a ranked list (one hue, values written out). */
export function CategoryBreakdown({ data, loading }: { data?: Dashboard; loading: boolean }) {
  const items = data?.expenses_by_category ?? [];
  const total = items.reduce((sum, i) => sum + i.amount, 0);
  const top = items.slice(0, 6);
  const rest = items.slice(6).reduce((sum, i) => sum + i.amount, 0);
  const rows = rest > 0 ? [...top, { account_id: 'other', name: 'Other categories', amount: rest }] : top;
  return (
    <SectionCard title="Spending by category" description={total > 0 && data ? `${formatMoney(total, data.currency)} in total` : undefined}>
      {loading && !data ? (
        <Skeleton height={220} />
      ) : rows.length === 0 ? (
        <Text size="sm" c="dimmed" py="xl" ta="center">
          No expenses in this period.
        </Text>
      ) : (
        <Stack gap="sm">
          {rows.map((r) => (
            <div key={r.account_id}>
              <Group justify="space-between" gap="xs" mb={4} wrap="nowrap">
                <Text size="sm" className="clamp-1">
                  {r.name}
                </Text>
                <Group gap={6} wrap="nowrap">
                  <MoneyText value={r.amount} currency={data?.currency} size="sm" fw={500} />
                  <Text size="xs" c="dimmed" w={36} ta="right" className="tabular">
                    {formatPercent(total ? r.amount / total : 0)}
                  </Text>
                </Group>
              </Group>
              <Progress value={total ? (r.amount / total) * 100 : 0} color="var(--app-expense)" size={6} radius="xl" aria-hidden />
            </div>
          ))}
        </Stack>
      )}
    </SectionCard>
  );
}

export function RecentInvoices({ data }: { data?: Dashboard }) {
  const invoices = data?.recent_invoices ?? [];
  return (
    <SectionCard
      title="Recent invoices"
      actions={
        <Text component={Link} to="/invoices" size="sm" c="brand">
          View all
        </Text>
      }
    >
      {invoices.length === 0 ? (
        <Text size="sm" c="dimmed">
          Invoices you create will appear here.
        </Text>
      ) : (
        <Stack gap={6}>
          {invoices.map((i) => (
            <UnstyledButton key={i.id} component={Link} to={`/invoices/${i.id}`}>
              <Group justify="space-between" wrap="nowrap" py={4}>
                <div style={{ minWidth: 0 }}>
                  <Text size="sm" fw={500} className="clamp-1">
                    {i.customer_name}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {i.number ?? 'Draft'} · due {formatDate(i.due_date)}
                  </Text>
                </div>
                <Group gap="xs" wrap="nowrap" style={{ flexShrink: 0 }}>
                  <MoneyText value={i.total} currency={i.currency} size="sm" />
                  <StatusBadge status={i.display_status} />
                </Group>
              </Group>
            </UnstyledButton>
          ))}
        </Stack>
      )}
    </SectionCard>
  );
}

export function GstCard({ data }: { data?: Dashboard }) {
  if (!data?.gst_registered) return null;
  const { collected, claimable, net } = data.gst;
  return (
    <SectionCard title="GST in this period" description="From your records. A guide for your return, not a filed return.">
      <Stack gap={6}>
        <Group justify="space-between">
          <Text size="sm" c="dimmed">
            Collected on sales
          </Text>
          <MoneyText value={collected} currency={data.currency} size="sm" />
        </Group>
        <Group justify="space-between">
          <Text size="sm" c="dimmed">
            Claimable on expenses
          </Text>
          <MoneyText value={claimable} currency={data.currency} size="sm" />
        </Group>
        <Group justify="space-between">
          <Text size="sm" fw={600}>
            {net >= 0 ? 'Likely to pay' : 'Likely refund'}
          </Text>
          <Group gap={6}>
            <MoneyText value={Math.abs(net)} currency={data.currency} fw={650} />
            <Badge color="gray" variant="outline">
              Estimate
            </Badge>
          </Group>
        </Group>
      </Stack>
    </SectionCard>
  );
}

/** Mileage in the period, when there are trips. The claims are already part of the expenses above. */
export function MileageCard({ data }: { data?: Dashboard }) {
  if (!data || data.mileage.trips === 0) return null;
  const { mileage } = data;
  return (
    <SectionCard
      title="Mileage"
      description="Claims are included in your expenses."
      actions={
        <Anchor component={Link} to="/mileage?tab=report" size="sm">
          View report
        </Anchor>
      }
    >
      <Stack gap={6}>
        <Group justify="space-between">
          <Text size="sm" c="dimmed">
            Business distance
          </Text>
          <Text size="sm" className="tabular">
            {formatKm(mileage.business_distance_km)}
          </Text>
        </Group>
        <Group justify="space-between">
          <Text size="sm" c="dimmed">
            Trips
          </Text>
          <Text size="sm" className="tabular">
            {formatNumber(mileage.trips)}
          </Text>
        </Group>
        <Group justify="space-between">
          <Text size="sm" fw={600}>
            Mileage claim
          </Text>
          <MoneyText value={mileage.claim_amount} currency={data.currency} fw={650} />
        </Group>
      </Stack>
    </SectionCard>
  );
}
