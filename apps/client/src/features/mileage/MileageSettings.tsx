import { ActionIcon, Badge, Button, Group, Stack, Table, Text, Tooltip, VisuallyHidden } from '@mantine/core';
import type { Business } from '@solo/api-client';
import { Archive, ArchiveRestore, CalendarPlus, Car, Gauge, Pencil, Plus, Star, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { BRAND } from '@/branding/brand';
import { confirmAction } from '@/components/confirmAction';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate, formatNumber } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import {
  mileageChangeKeys,
  useMileageRates,
  useVehicles,
  type FuelType,
  type MileageRate,
  type RateVersion,
  type Vehicle,
  type VehicleType,
} from './api';
import { formatRate, formatTiers, FUEL_TYPE_LABELS, VEHICLE_TYPE_LABELS, vehicleDescription } from './format';
import { NewRateModal, RateDetailsModal, RateVersionModal } from './RateModals';
import { VehicleFormModal } from './VehicleFormModal';
import { makeDefaultInput } from './vehicleForm';

const rowStyle = { borderBottom: '1px solid var(--app-border)' };

function count(n: number, one: string, many = `${one}s`): string {
  return `${formatNumber(n)} ${n === 1 ? one : many}`;
}

function ArchivedToggle({ shown, total, noun, onToggle }: { shown: boolean; total: number; noun: string; onToggle: () => void }) {
  if (total === 0) return null;
  return (
    <Group mt="sm">
      <Button variant="subtle" size="compact-sm" onClick={onToggle}>
        {shown ? `Hide archived ${noun}` : `Show archived ${noun} (${total})`}
      </Button>
    </Group>
  );
}

// ─── Vehicles ───────────────────────────────────────────────────────────────

function VehicleRow({
  vehicle,
  busy,
  onEdit,
  onMakeDefault,
  onArchive,
}: {
  vehicle: Vehicle;
  busy: boolean;
  onEdit: () => void;
  onMakeDefault: () => void;
  onArchive: (archived: boolean) => void;
}) {
  const description = vehicleDescription(vehicle);
  const kind = [VEHICLE_TYPE_LABELS[vehicle.vehicle_type as VehicleType], FUEL_TYPE_LABELS[vehicle.fuel_type as FuelType]].filter(Boolean);
  const trips = vehicle.trip_count === 0 ? 'No trips' : count(vehicle.trip_count, 'trip');
  return (
    <Group justify="space-between" wrap="nowrap" gap="sm" py={10} style={rowStyle}>
      <Stack gap={2} style={{ minWidth: 0 }}>
        <Group gap={6} wrap="nowrap">
          <Text size="sm" fw={500} truncate="end" c={vehicle.archived ? 'dimmed' : undefined}>
            {vehicle.name}
          </Text>
          {vehicle.is_default && <Badge variant="light">Default</Badge>}
          {vehicle.archived && (
            <Badge color="gray" variant="outline">
              Archived
            </Badge>
          )}
        </Group>
        <Text size="xs" c="dimmed">
          {[description, ...kind].filter(Boolean).join(' · ')}
        </Text>
        <Text size="xs" c="dimmed">
          {vehicle.default_rate_name ? `Rate: ${vehicle.default_rate_name}` : 'No default rate'} · {trips}
        </Text>
      </Stack>
      {
        <Group gap={2} wrap="nowrap">
          {!vehicle.archived && !vehicle.is_default && (
            <Tooltip label="Use for new trips">
              <ActionIcon aria-label={`Use ${vehicle.name} for new trips`} loading={busy} onClick={onMakeDefault}>
                <Star size={16} />
              </ActionIcon>
            </Tooltip>
          )}
          {!vehicle.archived && (
            <Tooltip label="Edit">
              <ActionIcon aria-label={`Edit ${vehicle.name}`} onClick={onEdit}>
                <Pencil size={16} />
              </ActionIcon>
            </Tooltip>
          )}
          <Tooltip label={vehicle.archived ? 'Restore' : 'Archive'}>
            <ActionIcon
              aria-label={`${vehicle.archived ? 'Restore' : 'Archive'} ${vehicle.name}`}
              loading={busy}
              onClick={() => onArchive(!vehicle.archived)}
            >
              {vehicle.archived ? <ArchiveRestore size={16} /> : <Archive size={16} />}
            </ActionIcon>
          </Tooltip>
        </Group>
      }
    </Group>
  );
}

function VehiclesSection({ business }: { business: Business }) {
  const vehicles = useVehicles();
  const [dialog, setDialog] = useState<{ opened: boolean; vehicle: Vehicle | null }>({ opened: false, vehicle: null });
  const [showArchived, setShowArchived] = useState(false);
  const invalidate = mileageChangeKeys(business.id);
  const archive = useApiMutation(
    ({ vehicle, archived }: { vehicle: Vehicle; archived: boolean }) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/mileage/vehicles/{vehicle_id}/archive', {
          params: { path: { business_id: business.id, vehicle_id: vehicle.id } },
          body: { archived },
        }),
      ),
    { successMessage: (row, { archived }) => (archived ? `${row.name} archived` : `${row.name} restored`), invalidate },
  );
  const makeDefault = useApiMutation(
    (vehicle: Vehicle) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/mileage/vehicles/{vehicle_id}', {
          params: { path: { business_id: business.id, vehicle_id: vehicle.id } },
          body: makeDefaultInput(vehicle),
        }),
      ),
    { successMessage: (row) => `New trips now start with ${row.name}`, invalidate },
  );
  const add = () => setDialog({ opened: true, vehicle: null });

  let body;
  if (vehicles.isPending) body = <ListSkeleton rows={3} height={56} />;
  else if (vehicles.isError) body = <ErrorState error={vehicles.error} onRetry={() => void vehicles.refetch()} />;
  else {
    const active = vehicles.data.filter((v) => !v.archived);
    const archived = vehicles.data.filter((v) => v.archived);
    const shown = showArchived ? [...active, ...archived] : active;
    const busyId = (archive.isPending && archive.variables?.vehicle.id) || (makeDefault.isPending && makeDefault.variables?.id);
    body = (
      <Stack gap={0}>
        {active.length === 0 && !showArchived && (
          <EmptyBlock
            compact
            icon={Car}
            title={vehicles.data.length === 0 ? 'No vehicles yet' : 'No active vehicles'}
            description={'Add the vehicle you use for work to start recording trips.'}
          />
        )}
        {shown.map((vehicle) => (
          <VehicleRow
            key={vehicle.id}
            vehicle={vehicle}
            busy={busyId === vehicle.id}
            onEdit={() => setDialog({ opened: true, vehicle })}
            onMakeDefault={() => makeDefault.mutate(vehicle)}
            onArchive={(next) => archive.mutate({ vehicle, archived: next })}
          />
        ))}
        <ArchivedToggle shown={showArchived} total={archived.length} noun="vehicles" onToggle={() => setShowArchived((v) => !v)} />
      </Stack>
    );
  }

  return (
    <SectionCard
      title="Vehicles"
      description="The vehicles you use for work. New trips start with the default vehicle and its rate. Archived vehicles keep their trips but can’t be chosen for new ones."
      actions={
        <Button size="xs" leftSection={<Plus size={14} />} onClick={add}>
          Add vehicle
        </Button>
      }
    >
      {body}
      <VehicleFormModal opened={dialog.opened} onClose={() => setDialog((d) => ({ ...d, opened: false }))} vehicle={dialog.vehicle} />
    </SectionCard>
  );
}

// ─── Mileage rates ──────────────────────────────────────────────────────────

function VersionTable({
  rate,
  currency,
  today,
  canEdit,
  onEdit,
  onDelete,
}: {
  rate: MileageRate;
  currency: string;
  today: string;
  canEdit: boolean;
  onEdit: (version: RateVersion) => void;
  onDelete: (version: RateVersion) => void;
}) {
  // Newest first; the API sends them oldest first.
  const versions = [...rate.versions].reverse();
  const deletable = canEdit && versions.length > 1;
  return (
    <Table withRowBorders={false} verticalSpacing={4} horizontalSpacing="xs" fz="sm" aria-label={`${rate.name} versions`}>
      <Table.Thead>
        <Table.Tr>
          <Table.Th fw={500} c="dimmed" fz="xs">
            Applies from
          </Table.Th>
          <Table.Th fw={500} c="dimmed" fz="xs" ta="right">
            Rate per km
          </Table.Th>
          {canEdit && (
            <Table.Th w={72}>
              <VisuallyHidden>Actions</VisuallyHidden>
            </Table.Th>
          )}
        </Table.Tr>
      </Table.Thead>
      <Table.Tbody>
        {versions.map((version) => {
          const from = formatDate(version.effective_from);
          return (
            <Table.Tr key={version.id}>
              <Table.Td>
                <Group gap={6} wrap="nowrap">
                  <Text size="sm">{from}</Text>
                  {rate.current?.id === version.id && (
                    <Badge size="xs" variant="light">
                      Current
                    </Badge>
                  )}
                  {version.effective_from > today && (
                    <Badge size="xs" color="gray" variant="light">
                      Upcoming
                    </Badge>
                  )}
                </Group>
              </Table.Td>
              <Table.Td ta="right" style={{ fontVariantNumeric: 'tabular-nums' }}>
                {formatRate(version.rate_per_km, currency)}
                {version.tier1_limit_km && (
                  <Text size="xs" c="dimmed">
                    then {formatTiers(version.tier1_limit_km, version.tier2_rate_per_km, currency)}
                  </Text>
                )}
              </Table.Td>
              {canEdit && (
                <Table.Td>
                  <Group gap={2} wrap="nowrap" justify="flex-end">
                    <Tooltip label="Edit">
                      <ActionIcon size="sm" aria-label={`Edit the rate from ${from}`} onClick={() => onEdit(version)}>
                        <Pencil size={14} />
                      </ActionIcon>
                    </Tooltip>
                    {deletable && (
                      <Tooltip label="Delete">
                        <ActionIcon size="sm" color="red" aria-label={`Delete the rate from ${from}`} onClick={() => onDelete(version)}>
                          <Trash2 size={14} />
                        </ActionIcon>
                      </Tooltip>
                    )}
                  </Group>
                </Table.Td>
              )}
            </Table.Tr>
          );
        })}
      </Table.Tbody>
    </Table>
  );
}

function RateRow({
  rate,
  business,
  busy,
  onEdit,
  onAddVersion,
  onEditVersion,
  onDeleteVersion,
  onArchive,
}: {
  rate: MileageRate;
  business: Business;
  busy: boolean;
  onEdit: () => void;
  onAddVersion: () => void;
  onEditVersion: (version: RateVersion) => void;
  onDeleteVersion: (version: RateVersion) => void;
  onArchive: (archived: boolean) => void;
}) {
  const details = [
    rate.current ? `${formatRate(rate.current.rate_per_km, business.currency)} today` : 'No rate in force today',
    rate.vehicle_count > 0 ? `default for ${count(rate.vehicle_count, 'vehicle')}` : null,
    rate.trip_count === 0 ? 'no trips' : count(rate.trip_count, 'trip'),
  ].filter(Boolean);
  return (
    <Stack gap="xs" py="sm" style={rowStyle}>
      <Group justify="space-between" wrap="nowrap" gap="sm" align="flex-start">
        <Stack gap={2} style={{ minWidth: 0 }}>
          <Group gap={6} wrap="nowrap">
            <Text size="sm" fw={500} truncate="end" c={rate.archived ? 'dimmed' : undefined}>
              {rate.name}
            </Text>
            {rate.archived && (
              <Badge color="gray" variant="outline">
                Archived
              </Badge>
            )}
          </Group>
          <Text size="xs" c="dimmed">
            {details.join(' · ')}
          </Text>
          {rate.notes && (
            <Text size="xs" c="dimmed" style={{ whiteSpace: 'pre-line' }}>
              {rate.notes}
            </Text>
          )}
        </Stack>
        {
          <Group gap={2} wrap="nowrap">
            {!rate.archived && (
              <Tooltip label="Add version">
                <ActionIcon aria-label={`Add a version to ${rate.name}`} onClick={onAddVersion}>
                  <CalendarPlus size={16} />
                </ActionIcon>
              </Tooltip>
            )}
            <Tooltip label="Edit name and notes">
              <ActionIcon aria-label={`Edit ${rate.name}`} onClick={onEdit}>
                <Pencil size={16} />
              </ActionIcon>
            </Tooltip>
            <Tooltip label={rate.archived ? 'Restore' : 'Archive'}>
              <ActionIcon
                aria-label={`${rate.archived ? 'Restore' : 'Archive'} ${rate.name}`}
                loading={busy}
                onClick={() => onArchive(!rate.archived)}
              >
                {rate.archived ? <ArchiveRestore size={16} /> : <Archive size={16} />}
              </ActionIcon>
            </Tooltip>
          </Group>
        }
      </Group>
      <VersionTable
        rate={rate}
        currency={business.currency}
        today={business.today}
        canEdit={!rate.archived}
        onEdit={onEditVersion}
        onDelete={onDeleteVersion}
      />
    </Stack>
  );
}

type RateDialog =
  { kind: 'new' } | { kind: 'details'; rate: MileageRate } | { kind: 'version'; rate: MileageRate; version: RateVersion | null };

function RatesSection({ business }: { business: Business }) {
  const rates = useMileageRates();
  const [dialog, setDialog] = useState<{ opened: boolean; target: RateDialog }>({ opened: false, target: { kind: 'new' } });
  const [showArchived, setShowArchived] = useState(false);
  const invalidate = mileageChangeKeys(business.id);
  const archive = useApiMutation(
    ({ rate, archived }: { rate: MileageRate; archived: boolean }) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/mileage/rates/{rate_id}/archive', {
          params: { path: { business_id: business.id, rate_id: rate.id } },
          body: { archived },
        }),
      ),
    { successMessage: (row, { archived }) => (archived ? `${row.name} archived` : `${row.name} restored`), invalidate },
  );
  const deleteVersion = useApiMutation(
    ({ rate, version }: { rate: MileageRate; version: RateVersion }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/mileage/rates/{rate_id}/versions/{version_id}', {
          params: { path: { business_id: business.id, rate_id: rate.id, version_id: version.id } },
        }),
      ),
    { successMessage: (row, { version }) => `${row.name}: the rate from ${formatDate(version.effective_from)} was deleted`, invalidate },
  );
  const open = (target: RateDialog) => setDialog({ opened: true, target });
  const close = () => setDialog((d) => ({ ...d, opened: false }));
  const confirmDelete = async (rate: MileageRate, version: RateVersion) => {
    const confirmed = await confirmAction({
      title: 'Delete this rate version?',
      message: `${formatRate(version.rate_per_km, business.currency)} from ${formatDate(version.effective_from)} will be removed from ${rate.name}. Trips you’ve already recorded keep the rate they used.`,
      confirmLabel: 'Delete version',
      danger: true,
    });
    if (confirmed) deleteVersion.mutate({ rate, version });
  };

  let body;
  if (rates.isPending) body = <ListSkeleton rows={2} height={96} />;
  else if (rates.isError) body = <ErrorState error={rates.error} onRetry={() => void rates.refetch()} />;
  else {
    const active = rates.data.filter((r) => !r.archived);
    const archived = rates.data.filter((r) => r.archived);
    const shown = showArchived ? [...active, ...archived] : active;
    body = (
      <Stack gap={0}>
        {active.length === 0 && !showArchived && (
          <EmptyBlock
            compact
            icon={Gauge}
            title={rates.data.length === 0 ? 'No mileage rates yet' : 'No active mileage rates'}
            description={'Add your rate per kilometre here, or when you add a vehicle.'}
          />
        )}
        {shown.map((rate) => (
          <RateRow
            key={rate.id}
            rate={rate}
            business={business}
            busy={archive.isPending && archive.variables?.rate.id === rate.id}
            onEdit={() => open({ kind: 'details', rate })}
            onAddVersion={() => open({ kind: 'version', rate, version: null })}
            onEditVersion={(version) => open({ kind: 'version', rate, version })}
            onDeleteVersion={(version) => void confirmDelete(rate, version)}
            onArchive={(next) => archive.mutate({ rate, archived: next })}
          />
        ))}
        <ArchivedToggle shown={showArchived} total={archived.length} noun="rates" onToggle={() => setShowArchived((v) => !v)} />
      </Stack>
    );
  }

  const { opened, target } = dialog;
  return (
    <SectionCard
      title="Mileage rates"
      description={`Your own rates per kilometre, for example the kilometre rate IRD publishes for the year: ${BRAND.name} doesn’t supply them. A trip uses the version in force on its date and keeps it, even if you change the rate later.`}
      actions={
        <Button size="xs" leftSection={<Plus size={14} />} onClick={() => open({ kind: 'new' })}>
          Add rate
        </Button>
      }
    >
      {body}
      <NewRateModal opened={opened && target.kind === 'new'} onClose={close} rates={rates.data ?? []} />
      <RateDetailsModal
        opened={opened && target.kind === 'details'}
        onClose={close}
        rate={target.kind === 'details' ? target.rate : null}
      />
      <RateVersionModal
        opened={opened && target.kind === 'version'}
        onClose={close}
        rate={target.kind === 'version' ? target.rate : null}
        version={target.kind === 'version' ? target.version : null}
      />
    </SectionCard>
  );
}

/** Settings section: vehicles and mileage rates (with their dated versions). */
export function MileageSettings() {
  const business = useCurrentBusiness();
  return (
    <Stack gap="lg" key={business.id}>
      <VehiclesSection business={business} />
      <RatesSection business={business} />
    </Stack>
  );
}
