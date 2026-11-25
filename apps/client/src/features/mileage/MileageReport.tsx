import { BarChart } from '@mantine/charts';
import { Box, Paper, SegmentedControl, SimpleGrid, Skeleton, Stack, Table, Text } from '@mantine/core';
import { useState } from 'react';
import { MoneyText } from '@/components/Money';
import { ErrorState, SectionCard } from '@/components/States';
import { formatNumber } from '@/lib/format';
import { monthLabel } from '@/lib/periods';
import { useMileageSummary, type MileagePeriod, type TripTotals } from './api';
import { formatKm } from './format';

function Tile({ label, value, hint }: { label: string; value: React.ReactNode; hint?: string }) {
  return (
    <Paper withBorder radius="lg" p="md">
      <Text size="xs" c="dimmed">
        {label}
      </Text>
      <Text fz="1.35rem" fw={650} className="tabular" mt={4}>
        {value}
      </Text>
      {hint && (
        <Text size="xs" c="dimmed" mt={2}>
          {hint}
        </Text>
      )}
    </Paper>
  );
}

function TotalsCells({ totals, currency }: { totals: TripTotals; currency: string }) {
  return (
    <>
      <Table.Td ta="right">{formatNumber(totals.trips)}</Table.Td>
      <Table.Td ta="right">{formatKm(totals.distance_km)}</Table.Td>
      <Table.Td ta="right">{formatKm(totals.business_distance_km)}</Table.Td>
      <Table.Td ta="right">{formatKm(totals.personal_distance_km)}</Table.Td>
      <Table.Td ta="right">
        <MoneyText value={totals.claim_amount} currency={currency} size="sm" />
      </Table.Td>
    </>
  );
}

function TotalsHeader({ first }: { first: string }) {
  return (
    <Table.Thead>
      <Table.Tr>
        <Table.Th>{first}</Table.Th>
        <Table.Th ta="right">Trips</Table.Th>
        <Table.Th ta="right">Total</Table.Th>
        <Table.Th ta="right">Business</Table.Th>
        <Table.Th ta="right">Personal</Table.Th>
        <Table.Th ta="right">Claim</Table.Th>
      </Table.Tr>
    </Table.Thead>
  );
}

/** Totals, kilometres by month (chart or table) and the breakdown by vehicle for a period. */
export function MileageReport({ period, currency }: { period: MileagePeriod; currency: string }) {
  const summary = useMileageSummary(period);
  const [view, setView] = useState<'chart' | 'table'>('chart');
  if (summary.isError) return <ErrorState error={summary.error} onRetry={() => void summary.refetch()} />;
  const data = summary.data;
  if (!data) return <Skeleton h={420} radius="lg" />;
  const { totals } = data;
  const rows = data.monthly.map((m) => ({ month: monthLabel(m.month), 'Business km': Number(m.business_distance_km), raw: m }));
  const noTrips = totals.trips === 0;

  return (
    <Stack gap="lg" style={{ opacity: summary.isPlaceholderData ? 0.6 : 1, transition: 'opacity 120ms' }}>
      <SimpleGrid cols={{ base: 2, sm: 3, lg: 5 }} spacing="md">
        <Tile label="Trips" value={formatNumber(totals.trips)} />
        <Tile label="Total distance" value={formatKm(totals.distance_km)} />
        <Tile label="Business" value={formatKm(totals.business_distance_km)} />
        <Tile label="Personal" value={formatKm(totals.personal_distance_km)} />
        <Tile
          label="Mileage claim"
          value={<MoneyText value={totals.claim_amount} currency={currency} inherit />}
          hint="Counted in your expenses"
        />
      </SimpleGrid>

      <SectionCard
        title="Business kilometres by month"
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
        {noTrips ? (
          <Box h={240} style={{ display: 'grid', placeItems: 'center' }}>
            <Text size="sm" c="dimmed">
              No trips in this period.
            </Text>
          </Box>
        ) : view === 'chart' ? (
          <BarChart
            h={240}
            data={rows}
            dataKey="month"
            series={[{ name: 'Business km', color: 'var(--app-series-1)' }]}
            gridAxis="y"
            tickLine="none"
            barProps={{ radius: [4, 4, 0, 0] }}
            maxBarWidth={28}
            valueFormatter={(v) => formatKm(v)}
            yAxisProps={{ tickFormatter: (v: number) => formatNumber(v), width: 56 }}
          />
        ) : (
          <Table.ScrollContainer minWidth={560}>
            <Table>
              <TotalsHeader first="Month" />
              <Table.Tbody>
                {data.monthly.map((m) => (
                  <Table.Tr key={m.month}>
                    <Table.Td>{monthLabel(m.month)}</Table.Td>
                    <TotalsCells totals={m} currency={currency} />
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}
      </SectionCard>

      <SectionCard title="By vehicle">
        {data.vehicles.length === 0 ? (
          <Text size="sm" c="dimmed">
            No trips in this period.
          </Text>
        ) : (
          <Table.ScrollContainer minWidth={560}>
            <Table>
              <TotalsHeader first="Vehicle" />
              <Table.Tbody>
                {data.vehicles.map((v) => (
                  <Table.Tr key={v.vehicle_id}>
                    <Table.Td>{v.vehicle_name}</Table.Td>
                    <TotalsCells totals={v} currency={currency} />
                  </Table.Tr>
                ))}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
        )}
      </SectionCard>
    </Stack>
  );
}
