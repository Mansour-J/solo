import {
  Alert,
  Button,
  Checkbox,
  Group,
  Modal,
  Paper,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Text,
  Textarea,
  TextInput,
} from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { useDisclosure } from '@mantine/hooks';
import { Car, Info, Trash2 } from 'lucide-react';
import { MoneyText } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { isApiError } from '@/services/api/errors';
import { useApiMutation } from '@/services/api/query';
import {
  mileageChangeKeys,
  useFrequentTrips,
  useMileageRates,
  useVehicles,
  type MileageRate,
  type Trip,
  type TripDetail,
  type Vehicle,
} from './api';
import { formatKm, formatRate, formatTiers } from './format';
import {
  calculationInput,
  CUSTOM_RATE,
  defaultVehicle,
  emptyTrip,
  fromFrequent,
  frequentLabel,
  fromTrip,
  toTripInput,
  tripSchema,
  VEHICLE_RATE,
  type TripFormValues,
} from './tripForm';
import { useTripCalculation } from './useTripCalculation';
import { VehicleFormModal } from './VehicleFormModal';

export interface TripFormModalProps {
  opened: boolean;
  onClose: () => void;
  /** The trip to edit; omit to record a new one. */
  trip?: Trip | null;
  /** Called with the saved trip after the modal closes. */
  onSaved?: (trip: TripDetail) => void;
}

/** Record or edit a trip. With no vehicle yet, it offers to add one first. */
export function TripFormModal({ opened, onClose, trip, onSaved }: TripFormModalProps) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={trip ? 'Edit trip' : 'Record a trip'} size="lg" fullScreen={isMobile}>
      <TripFormLoader key={trip?.id ?? 'new'} trip={trip ?? null} onClose={onClose} onSaved={onSaved} />
    </Modal>
  );
}

function TripFormLoader({ trip, onClose, onSaved }: { trip: Trip | null; onClose: () => void; onSaved?: (trip: TripDetail) => void }) {
  const vehicles = useVehicles();
  const rates = useMileageRates();
  const [addingVehicle, vehicleModal] = useDisclosure(false);
  if (vehicles.isPending || rates.isPending) return <ListSkeleton rows={6} height={44} />;
  if (vehicles.isError) return <ErrorState error={vehicles.error} onRetry={() => void vehicles.refetch()} />;
  if (rates.isError) return <ErrorState error={rates.error} onRetry={() => void rates.refetch()} />;
  const first = defaultVehicle(vehicles.data);
  if (!trip && !first) {
    return (
      <>
        <EmptyBlock
          icon={Car}
          title="Add a vehicle first"
          description="Trips are recorded against a vehicle, with the rate per kilometre you claim for it."
          action={<Button onClick={vehicleModal.open}>Add vehicle</Button>}
        />
        <VehicleFormModal opened={addingVehicle} onClose={vehicleModal.close} />
      </>
    );
  }
  return (
    <TripForm trip={trip} vehicles={vehicles.data} rates={rates.data} initialVehicle={first ?? null} onClose={onClose} onSaved={onSaved} />
  );
}

function rateLabel(rate: MileageRate, currency: string): string {
  return rate.current ? `${rate.name} · ${formatRate(rate.current.rate_per_km, currency)}` : rate.name;
}

function TripForm({
  trip,
  vehicles,
  rates,
  initialVehicle,
  onClose,
  onSaved,
}: {
  trip: Trip | null;
  vehicles: Vehicle[];
  rates: MileageRate[];
  initialVehicle: Vehicle | null;
  onClose: () => void;
  onSaved?: (trip: TripDetail) => void;
}) {
  const business = useCurrentBusiness();
  const currency = business.currency;
  const form = useForm<TripFormValues>({
    initialValues: trip ? fromTrip(trip) : emptyTrip(business.today, initialVehicle?.id ?? null),
    validate: zodValidate<TripFormValues>(tripSchema),
  });
  const invalidate = mileageChangeKeys(business.id);
  const frequent = useFrequentTrips();
  const frequentOptions = (frequent.data ?? []).map((f, i) => ({ value: String(i), label: frequentLabel(f) }));
  const save = useApiMutation(
    (values: TripFormValues) => {
      const body = toTripInput(values);
      return trip
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/mileage/trips/{trip_id}', {
              params: { path: { business_id: business.id, trip_id: trip.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/mileage/trips', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: trip ? 'Trip updated' : 'Trip recorded',
      invalidate,
      onSuccess: (saved) => {
        onClose();
        onSaved?.(saved);
      },
    },
  );
  const remove = useApiMutation(
    ({ id, reason }: { id: string; reason: string }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/mileage/trips/{trip_id}', {
          params: { path: { business_id: business.id, trip_id: id } },
          body: { reason },
        }),
      ),
    { successMessage: 'Trip deleted', invalidate, onSuccess: onClose },
  );

  const values = form.getValues();
  const { calculation, error } = useTripCalculation(business.id, calculationInput(values), trip?.id);
  const calculationProblem = isApiError(error) ? (Object.values(error.fields)[0] ?? error.message) : null;

  const vehicle = vehicles.find((v) => v.id === values.vehicle_id);
  const vehicleOptions = vehicles
    .filter((v) => !v.archived || v.id === trip?.vehicle_id)
    .map((v) => ({ value: v.id, label: v.archived ? `${v.name} (archived)` : v.name }));
  const rateOptions = [
    {
      value: VEHICLE_RATE,
      label: vehicle?.default_rate_name ? `Vehicle’s rate (${vehicle.default_rate_name})` : 'Vehicle’s rate (none set)',
    },
    ...rates
      .filter((r) => !r.archived || r.id === trip?.rate_id)
      .map((r) => ({ value: r.id, label: r.archived ? `${rateLabel(r, currency)} (archived)` : rateLabel(r, currency) })),
    { value: CUSTOM_RATE, label: 'Enter a rate for this trip' },
  ];

  const confirmDelete = async (row: Trip) => {
    const reason = await promptReason({
      title: 'Delete this trip?',
      message: `The trip of ${formatKm(row.distance_km)} on ${formatDate(row.trip_date)} is removed from your mileage and its claim is reversed in your books. It stays in the history with your reason.`,
      confirmLabel: 'Delete trip',
      danger: true,
    });
    if (reason !== null) remove.mutate({ id: row.id, reason });
  };

  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="md">
        {!trip && frequentOptions.length > 0 && (
          <Select
            label="Frequent trips"
            description="Fill in a trip you make often, then check the date."
            placeholder="Choose a trip"
            data={frequentOptions}
            value={null}
            onChange={(i) => {
              const chosen = i === null ? undefined : frequent.data?.[Number(i)];
              if (chosen) form.setValues(fromFrequent(form.getValues(), chosen));
            }}
          />
        )}
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <Select label="Vehicle" withAsterisk allowDeselect={false} data={vehicleOptions} {...form.getInputProps('vehicle_id')} />
          <DateInput label="Date" withAsterisk valueFormat={dateInputFormat()} {...form.getInputProps('trip_date')} />
        </SimpleGrid>
        <TextInput
          label="Purpose"
          placeholder="e.g. Client meeting with Kererū Café"
          withAsterisk
          maxLength={300}
          data-autofocus
          {...form.getInputProps('purpose')}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput label="From" placeholder="Start location" maxLength={200} {...form.getInputProps('start_location')} />
          <TextInput label="To" placeholder="Destination" maxLength={200} {...form.getInputProps('destination')} />
        </SimpleGrid>

        <Stack gap="xs">
          <Group justify="space-between" gap="xs">
            <Text size="sm" fw={500}>
              Distance
            </Text>
            <SegmentedControl
              size="xs"
              aria-label="How you’ll enter the distance"
              data={[
                { value: 'distance', label: 'Kilometres' },
                { value: 'odometer', label: 'Odometer readings' },
              ]}
              {...form.getInputProps('distance_mode')}
            />
          </Group>
          {values.distance_mode === 'odometer' ? (
            <SimpleGrid cols={2}>
              <TextInput
                label="Start reading"
                inputMode="decimal"
                rightSection={<Text size="xs">km</Text>}
                {...form.getInputProps('start_odometer')}
              />
              <TextInput
                label="End reading"
                inputMode="decimal"
                rightSection={<Text size="xs">km</Text>}
                {...form.getInputProps('end_odometer')}
              />
            </SimpleGrid>
          ) : (
            <Stack gap={6}>
              <TextInput
                label="Kilometres"
                description={values.return_trip ? 'One way: the trip counts it there and back.' : 'Total distance travelled.'}
                inputMode="decimal"
                rightSection={<Text size="xs">km</Text>}
                {...form.getInputProps('distance_km')}
              />
              <Checkbox label="Return trip (there and back)" {...form.getInputProps('return_trip', { type: 'checkbox' })} />
            </Stack>
          )}
        </Stack>

        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput
            label="Business share"
            description="How much of the trip was for the business."
            inputMode="decimal"
            rightSection={<Text size="xs">%</Text>}
            {...form.getInputProps('business_percentage')}
          />
          <Select label="Rate" allowDeselect={false} data={rateOptions} {...form.getInputProps('rate_id')} />
        </SimpleGrid>
        {values.rate_id === CUSTOM_RATE && (
          <TextInput
            label="Rate per km"
            description="Recorded with this trip only."
            inputMode="decimal"
            leftSection={<Text size="xs">$</Text>}
            w={{ base: '100%', sm: 220 }}
            {...form.getInputProps('rate_per_km')}
          />
        )}

        <Paper withBorder radius="md" p="sm" aria-live="polite">
          <Text size="xs" c="dimmed" mb={6}>
            How this claim is worked out
          </Text>
          {calculation ? (
            <Stack gap={4}>
              <Group justify="space-between" align="baseline" gap="xs">
                <Text size="sm">
                  {Number(calculation.tier2_distance_km) > 0
                    ? `${formatKm(Number(calculation.business_distance_km) - Number(calculation.tier2_distance_km))} × ${formatRate(calculation.rate_per_km, currency)} + ${formatKm(calculation.tier2_distance_km)} × ${formatRate(calculation.tier2_rate_per_km, currency)}`
                    : `${formatKm(calculation.business_distance_km)} × ${formatRate(calculation.rate_per_km, currency)}`}
                </Text>
                <MoneyText value={calculation.claim_amount} currency={currency} fw={650} />
              </Group>
              <Text size="xs" c="dimmed">
                {calculation.rate_overridden
                  ? 'Rate entered for this trip.'
                  : `${calculation.rate_name ?? 'Rate'} in force from ${formatDate(calculation.rate_effective_from)}.`}{' '}
                {Number(calculation.personal_distance_km) > 0 ? `${formatKm(calculation.personal_distance_km)} personal, not claimed.` : ''}
              </Text>
              {calculation.tier1_limit_km && (
                <Text size="xs" c="dimmed">
                  {formatKm(calculation.travelled_before_km)} recorded for this vehicle earlier in the financial year;{' '}
                  {formatTiers(calculation.tier1_limit_km, calculation.tier2_rate_per_km, currency)}. Only trips you record count toward the
                  limit, so record private trips too.
                </Text>
              )}
            </Stack>
          ) : calculationProblem ? (
            <Text size="sm" c="orange.8">
              {calculationProblem}
            </Text>
          ) : (
            <Text size="sm" c="dimmed">
              Enter the distance to see the claim.
            </Text>
          )}
        </Paper>
        <Alert variant="light" color="gray" icon={<Info size={16} />} p="xs">
          <Text size="xs">
            A kilometre rate usually covers the vehicle’s running costs, so check with your accountant before also recording its fuel or
            repairs as expenses.
          </Text>
        </Alert>

        <Textarea label="Notes" autosize minRows={2} maxRows={6} maxLength={2000} {...form.getInputProps('notes')} />

        <Group justify="space-between" gap="sm">
          {trip ? (
            <Button
              variant="subtle"
              color="red"
              leftSection={<Trash2 size={16} />}
              loading={remove.isPending}
              onClick={() => void confirmDelete(trip)}
            >
              Delete
            </Button>
          ) : (
            <span />
          )}
          <Group gap="sm">
            <Button variant="default" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" loading={save.isPending}>
              {trip ? 'Save changes' : 'Record trip'}
            </Button>
          </Group>
        </Group>
      </Stack>
    </form>
  );
}
