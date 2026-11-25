import { Box, Button, Group, Pagination, Paper, Select, Stack, Table, Tabs, Text, TextInput, UnstyledButton } from '@mantine/core';
import { useDebouncedValue, useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import type { Business } from '@solo/api-client';
import { Car, Download, Plus, Route, Search } from 'lucide-react';
import { useState } from 'react';
import { useSearchParams } from 'react-router';
import { BRAND } from '@/branding/brand';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate, formatNumber } from '@/lib/format';
import { periodRange, type Period } from '@/lib/periods';
import { errorMessage } from '@/services/api/errors';
import { downloadMileageCsv, useTrips, useVehicles, type MileagePeriod, type Trip, type Vehicle } from './api';
import { formatKm, formatRate, formatShare } from './format';
import { MileageReport } from './MileageReport';
import { TripFormModal } from './TripFormModal';
import { VehicleFormModal } from './VehicleFormModal';

const PER_PAGE = 25;

function route(trip: Trip): string {
  return [trip.start_location, trip.destination].filter(Boolean).join(' → ');
}

function rateText(trip: Trip): string {
  const rate = formatRate(trip.rate_per_km, trip.currency);
  if (trip.rate_overridden) return `${rate} (entered)`;
  return Number(trip.tier2_distance_km) > 0
    ? `${rate}; ${formatKm(trip.tier2_distance_km)} at ${formatRate(trip.tier2_rate_per_km, trip.currency)}`
    : rate;
}

function TripTable({ items, onOpen }: { items: Trip[]; onOpen?: (trip: Trip) => void }) {
  return (
    <Paper withBorder radius="lg" style={{ overflow: 'hidden' }}>
      <Table.ScrollContainer minWidth={860}>
        <Table highlightOnHover={!!onOpen}>
          <Table.Thead>
            <Table.Tr>
              <Table.Th w={110}>Date</Table.Th>
              <Table.Th>Purpose</Table.Th>
              <Table.Th>Vehicle</Table.Th>
              <Table.Th ta="right">Distance</Table.Th>
              <Table.Th ta="right">Business</Table.Th>
              <Table.Th ta="right">Rate</Table.Th>
              <Table.Th ta="right">Claim</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((trip) => (
              <Table.Tr key={trip.id} onClick={onOpen ? () => onOpen(trip) : undefined} style={onOpen ? { cursor: 'pointer' } : undefined}>
                <Table.Td style={{ whiteSpace: 'nowrap' }}>{formatDate(trip.trip_date)}</Table.Td>
                <Table.Td maw={340}>
                  {/* Focusable for keyboard users; its click bubbles to the row. */}
                  <UnstyledButton display="block" maw="100%" aria-label={`Trip on ${formatDate(trip.trip_date)}: ${trip.purpose}`}>
                    <Text size="sm" fw={500} truncate="end">
                      {trip.purpose}
                    </Text>
                    {route(trip) && (
                      <Text size="xs" c="dimmed" truncate="end">
                        {route(trip)}
                      </Text>
                    )}
                  </UnstyledButton>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{trip.vehicle_name}</Text>
                </Table.Td>
                <Table.Td ta="right" className="tabular">
                  {formatKm(trip.distance_km)}
                </Table.Td>
                <Table.Td ta="right">
                  <Text size="sm" className="tabular">
                    {formatKm(trip.business_distance_km)}
                  </Text>
                  {Number(trip.business_percentage) < 100 && (
                    <Text size="xs" c="dimmed">
                      {formatShare(trip.business_percentage)}
                    </Text>
                  )}
                </Table.Td>
                <Table.Td ta="right">
                  <Text size="sm" className="tabular">
                    {rateText(trip)}
                  </Text>
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={trip.claim_amount} currency={trip.currency} size="sm" fw={500} />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
    </Paper>
  );
}

function TripCards({ items, onOpen }: { items: Trip[]; onOpen?: (trip: Trip) => void }) {
  return (
    <Stack gap="xs">
      {items.map((trip) => {
        const card = (
          <Paper withBorder radius="md" p="sm">
            <Group justify="space-between" wrap="nowrap" gap="sm" align="flex-start">
              <Box style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} truncate="end">
                  {trip.purpose}
                </Text>
                <Text size="xs" c="dimmed" truncate="end">
                  {formatDate(trip.trip_date)} · {trip.vehicle_name}
                </Text>
                {route(trip) && (
                  <Text size="xs" c="dimmed" truncate="end">
                    {route(trip)}
                  </Text>
                )}
              </Box>
              <Stack gap={2} align="flex-end" style={{ flexShrink: 0 }}>
                <MoneyText value={trip.claim_amount} currency={trip.currency} size="sm" fw={600} />
                <Text size="xs" c="dimmed" className="tabular">
                  {formatKm(trip.business_distance_km)} of {formatKm(trip.distance_km)}
                </Text>
              </Stack>
            </Group>
          </Paper>
        );
        return onOpen ? (
          <UnstyledButton key={trip.id} onClick={() => onOpen(trip)} aria-label={`Trip on ${formatDate(trip.trip_date)}: ${trip.purpose}`}>
            {card}
          </UnstyledButton>
        ) : (
          <Box key={trip.id}>{card}</Box>
        );
      })}
    </Stack>
  );
}

function Trips({
  business,
  period,
  vehicleId,
  onOpen,
}: {
  business: Business;
  period: Period;
  vehicleId: string | null;
  onOpen?: (trip: Trip) => void;
}) {
  const isMobile = useIsMobile();
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search.trim(), 300);
  const [page, setPage] = useState(1);
  const trips = useTrips({
    from: period.from,
    to: period.to,
    vehicle_id: vehicleId ?? undefined,
    search: debouncedSearch || undefined,
    page,
    per_page: PER_PAGE,
  });

  let content;
  if (trips.isPending) content = <ListSkeleton rows={6} height={isMobile ? 76 : 52} />;
  else if (trips.isError) content = <ErrorState error={trips.error} onRetry={() => void trips.refetch()} />;
  else if (trips.data.items.length === 0) {
    content = debouncedSearch ? (
      <EmptyBlock
        icon={Search}
        title="No trips match this search"
        action={
          <Button variant="default" onClick={() => setSearch('')}>
            Clear search
          </Button>
        }
      />
    ) : (
      <EmptyBlock
        icon={Route}
        title="No trips in this period"
        description="Record each business trip as you make it, or from your logbook."
      />
    );
  } else {
    const { items, totals } = trips.data;
    content = (
      <Stack gap="sm" style={{ opacity: trips.isPlaceholderData ? 0.6 : 1, transition: 'opacity 120ms' }}>
        <Group gap="lg" wrap="wrap">
          <Text size="sm" c="dimmed">
            {formatNumber(totals.trips)} {totals.trips === 1 ? 'trip' : 'trips'}
          </Text>
          <Text size="sm">
            Business <b className="tabular">{formatKm(totals.business_distance_km)}</b> of {formatKm(totals.distance_km)}
          </Text>
          <Text size="sm">
            Claim <MoneyText value={totals.claim_amount} currency={business.currency} fw={600} />
          </Text>
        </Group>
        {isMobile ? <TripCards items={items} onOpen={onOpen} /> : <TripTable items={items} onOpen={onOpen} />}
        {totals.trips > PER_PAGE && (
          <Group justify="flex-end">
            <Pagination total={Math.ceil(totals.trips / PER_PAGE)} value={page} onChange={setPage} size="sm" />
          </Group>
        )}
      </Stack>
    );
  }

  return (
    <Stack gap="md">
      <TextInput
        aria-label="Search trips"
        placeholder="Search purpose, places or notes"
        leftSection={<Search size={15} />}
        w={{ base: '100%', xs: 320 }}
        value={search}
        onChange={(event) => {
          setSearch(event.currentTarget.value);
          setPage(1);
        }}
      />
      {content}
    </Stack>
  );
}

function SetUp() {
  const [opened, modal] = useDisclosure(false);
  return (
    <>
      <EmptyBlock
        icon={Car}
        title="Track your business mileage"
        description={`Add the vehicle you use for work and the rate per kilometre you claim for it. Then record each trip, and ${BRAND.name} works out the claim and adds it to your expenses.`}
        action={<Button onClick={modal.open}>Add vehicle</Button>}
      />
      <VehicleFormModal opened={opened} onClose={modal.close} />
    </>
  );
}

function MileageView({ business, vehicles }: { business: Business; vehicles: Vehicle[] }) {
  const [searchParams, setSearchParams] = useSearchParams();
  const tab = searchParams.get('tab') === 'report' ? 'report' : 'trips';
  const creating = searchParams.get('new') === '1';
  const [editing, setEditing] = useState<Trip | null>(null);
  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const [vehicleId, setVehicleId] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const updateParams = (change: (next: URLSearchParams) => void) =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        change(next);
        return next;
      },
      { replace: true },
    );
  const reportPeriod: MileagePeriod = { from: period.from, to: period.to, ...(vehicleId ? { vehicle_id: vehicleId } : {}) };

  const exportCsv = async () => {
    setExporting(true);
    try {
      await downloadMileageCsv(business.id, reportPeriod);
    } catch (error) {
      notifications.show({ color: 'red', title: 'Couldn’t export the trips', message: errorMessage(error) });
    } finally {
      setExporting(false);
    }
  };

  const vehicleOptions = vehicles.map((v) => ({ value: v.id, label: v.archived ? `${v.name} (archived)` : v.name }));
  const hasActiveVehicle = vehicles.some((v) => !v.archived);

  return (
    <Page
      title="Mileage"
      description="Business trips, the kilometres you travel and what you can claim for them."
      actions={
        <Group gap="xs">
          <Button variant="default" leftSection={<Download size={16} />} loading={exporting} onClick={() => void exportCsv()}>
            Export CSV
          </Button>
          {hasActiveVehicle && (
            <Button leftSection={<Plus size={16} />} onClick={() => updateParams((p) => p.set('new', '1'))}>
              Record trip
            </Button>
          )}
        </Group>
      }
    >
      <Group gap="xs" wrap="wrap" align="flex-start">
        <PeriodPicker value={period} onChange={setPeriod} today={business.today} fyStart={business.current_financial_year.start} />
        {vehicles.length > 1 && (
          <Select
            aria-label="Vehicle"
            placeholder="All vehicles"
            clearable
            w={{ base: '100%', xs: 220 }}
            data={vehicleOptions}
            value={vehicleId}
            onChange={setVehicleId}
          />
        )}
      </Group>
      <Tabs
        value={tab}
        onChange={(value) =>
          updateParams((p) => {
            if (value === 'report') p.set('tab', 'report');
            else p.delete('tab');
          })
        }
      >
        <Tabs.List mb="md">
          <Tabs.Tab value="trips">Trips</Tabs.Tab>
          <Tabs.Tab value="report">Report</Tabs.Tab>
        </Tabs.List>
        <Tabs.Panel value="trips">
          <Trips business={business} period={period} vehicleId={vehicleId} onOpen={setEditing} />
        </Tabs.Panel>
        <Tabs.Panel value="report">{tab === 'report' && <MileageReport period={reportPeriod} currency={business.currency} />}</Tabs.Panel>
      </Tabs>
      <TripFormModal opened={creating} onClose={() => updateParams((p) => p.delete('new'))} />
      <TripFormModal opened={editing !== null} onClose={() => setEditing(null)} trip={editing} />
      {!hasActiveVehicle && (
        <Text size="sm" c="dimmed">
          All your vehicles are archived. Restore one in Settings › Mileage to record trips.
        </Text>
      )}
    </Page>
  );
}

/** Trips and the mileage report. `?new=1` opens the trip form; `?tab=report` shows the report. */
export function MileagePage() {
  const business = useCurrentBusiness();
  const vehicles = useVehicles();
  if (vehicles.isPending) {
    return (
      <Page title="Mileage">
        <ListSkeleton rows={6} />
      </Page>
    );
  }
  if (vehicles.isError) {
    return (
      <Page title="Mileage">
        <ErrorState error={vehicles.error} onRetry={() => void vehicles.refetch()} />
      </Page>
    );
  }
  if (vehicles.data.length === 0) {
    return (
      <Page title="Mileage" description="Business trips, the kilometres you travel and what you can claim for them.">
        <SetUp />
      </Page>
    );
  }
  return <MileageView key={business.id} business={business} vehicles={vehicles.data} />;
}
