import { Button, Group, Modal, NumberInput, Paper, Select, SimpleGrid, Stack, Switch, Text, Textarea, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import type { Business } from '@solo/api-client';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { mileageChangeKeys, useMileageRates, useVehicles, type MileageRate, type Vehicle } from './api';
import { FUEL_TYPE_OPTIONS, VEHICLE_TYPE_OPTIONS } from './format';
import { RatePerKmInput, RateSourceNote, TierInputs } from './RateInputs';
import { defaultRateName, emptyNewRateValues, rateOptionLabel } from './rateForm';
import { emptyVehicleValues, fromVehicle, NEW_RATE_OPTION, toVehicleInput, vehicleSchema, type VehicleFormValues } from './vehicleForm';

/** Add a vehicle (no `vehicle`) or edit one. Works with no vehicles and no rates yet: it can create the rate too. */
export function VehicleFormModal({
  opened,
  onClose,
  vehicle,
  onSaved,
}: {
  opened: boolean;
  onClose: () => void;
  vehicle?: Vehicle | null;
  onSaved?: (vehicle: Vehicle) => void;
}) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={vehicle ? `Edit ${vehicle.name}` : 'Add vehicle'} size="lg" fullScreen={isMobile}>
      <VehicleFormLoader key={vehicle?.id ?? 'new'} vehicle={vehicle ?? null} onClose={onClose} onSaved={onSaved} />
    </Modal>
  );
}

function VehicleFormLoader({
  vehicle,
  onClose,
  onSaved,
}: {
  vehicle: Vehicle | null;
  onClose: () => void;
  onSaved?: (vehicle: Vehicle) => void;
}) {
  const business = useCurrentBusiness();
  const rates = useMileageRates();
  const vehicles = useVehicles();
  if (rates.isPending || vehicles.isPending) return <ListSkeleton rows={6} height={44} />;
  if (rates.isError) return <ErrorState error={rates.error} onRetry={() => void rates.refetch()} />;
  if (vehicles.isError) return <ErrorState error={vehicles.error} onRetry={() => void vehicles.refetch()} />;
  return (
    <VehicleForm
      business={business}
      rates={rates.data}
      firstVehicle={!vehicles.data.some((v) => !v.archived)}
      vehicle={vehicle}
      onClose={onClose}
      onSaved={onSaved}
    />
  );
}

function defaultVehicleHelp(vehicle: Vehicle | null, firstVehicle: boolean): { locked: boolean; text: string } {
  if (!vehicle && firstVehicle) return { locked: true, text: 'Your first vehicle is always the one new trips start with.' };
  if (vehicle?.is_default) return { locked: true, text: 'To change this, make another vehicle the default.' };
  return { locked: false, text: 'New trips start with this vehicle.' };
}

function VehicleForm({
  business,
  rates,
  firstVehicle,
  vehicle,
  onClose,
  onSaved,
}: {
  business: Business;
  rates: MileageRate[];
  firstVehicle: boolean;
  vehicle: Vehicle | null;
  onClose: () => void;
  onSaved?: (vehicle: Vehicle) => void;
}) {
  const newRateFrom = business.current_financial_year.start;
  const form = useForm<VehicleFormValues>({
    initialValues: vehicle
      ? fromVehicle(vehicle, emptyNewRateValues(newRateFrom, defaultRateName(rates)))
      : emptyVehicleValues({ rates, newRateFrom, firstVehicle }),
    validate: zodValidate<VehicleFormValues>(vehicleSchema),
  });
  const save = useApiMutation(
    (values: VehicleFormValues) => {
      const body = toVehicleInput(values);
      return vehicle
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/mileage/vehicles/{vehicle_id}', {
              params: { path: { business_id: business.id, vehicle_id: vehicle.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/mileage/vehicles', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: (saved) => (vehicle ? `${saved.name} saved` : `${saved.name} added`),
      invalidate: mileageChangeKeys(business.id),
      onSuccess: (saved) => {
        onClose();
        onSaved?.(saved);
      },
    },
  );

  const v = form.values;
  // Archived rates can't be chosen, but a vehicle may keep the one it already has.
  const rateOptions = [
    ...rates
      .filter((r) => !r.archived || r.id === vehicle?.default_rate_id)
      .map((r) => ({ value: r.id, label: rateOptionLabel(r, business.currency) })),
    { value: NEW_RATE_OPTION, label: 'New rate…' },
  ];
  const defaultHelp = defaultVehicleHelp(vehicle, firstVehicle);

  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <TextInput
          label="Name"
          description="How the vehicle is shown on trips, such as “Work ute”."
          withAsterisk
          data-autofocus
          maxLength={80}
          {...form.getInputProps('name')}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput label="Registration" maxLength={12} autoComplete="off" {...form.getInputProps('registration')} />
          <NumberInput
            label="Year"
            min={1900}
            max={2100}
            allowDecimal={false}
            allowNegative={false}
            hideControls
            {...form.getInputProps('year')}
          />
          <TextInput label="Make" placeholder="e.g. Toyota" maxLength={60} {...form.getInputProps('make')} />
          <TextInput label="Model" placeholder="e.g. Hilux" maxLength={60} {...form.getInputProps('model')} />
          <Select label="Type" data={VEHICLE_TYPE_OPTIONS} allowDeselect={false} {...form.getInputProps('vehicle_type')} />
          <Select label="Fuel" data={FUEL_TYPE_OPTIONS} allowDeselect={false} {...form.getInputProps('fuel_type')} />
        </SimpleGrid>

        <Stack gap="xs">
          <Select
            label="Default rate"
            description="New trips with this vehicle use this rate unless you choose another."
            placeholder="No default rate"
            clearable
            data={rateOptions}
            value={v.create_rate ? NEW_RATE_OPTION : v.default_rate_id}
            onChange={(value) => {
              if (value === NEW_RATE_OPTION) {
                form.setFieldValue('create_rate', true);
              } else {
                form.setValues({ create_rate: false, default_rate_id: value });
              }
              form.clearFieldError('default_rate_id');
            }}
            error={form.errors.default_rate_id}
          />
          {v.create_rate && (
            <Paper withBorder radius="md" p="md">
              <Stack gap="sm">
                <Text size="sm" fw={500}>
                  New rate
                </Text>
                <RateSourceNote />
                <SimpleGrid cols={{ base: 1, sm: 3 }}>
                  <TextInput
                    label="Rate name"
                    description="Shown on trips and exports."
                    withAsterisk
                    maxLength={80}
                    {...form.getInputProps('new_rate.name')}
                  />
                  <RatePerKmInput currency={business.currency} withAsterisk {...form.getInputProps('new_rate.rate_per_km')} />
                  <DateInput
                    label="Applies from"
                    description="Trips from this date use it."
                    withAsterisk
                    valueFormat={dateInputFormat()}
                    {...form.getInputProps('new_rate.effective_from')}
                  />
                </SimpleGrid>
                <TierInputs
                  currency={business.currency}
                  limitProps={form.getInputProps('new_rate.tier1_limit_km')}
                  rateProps={form.getInputProps('new_rate.tier2_rate_per_km')}
                />
              </Stack>
            </Paper>
          )}
        </Stack>

        <Switch
          label="Use for new trips"
          description={defaultHelp.text}
          disabled={defaultHelp.locked}
          {...form.getInputProps('is_default', { type: 'checkbox' })}
        />
        <Textarea label="Notes" description="For your own reference." autosize minRows={2} maxRows={6} {...form.getInputProps('notes')} />

        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {vehicle ? 'Save changes' : 'Add vehicle'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
