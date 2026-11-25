import { Alert, Button, Group, Modal, SimpleGrid, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { Info } from 'lucide-react';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { mileageChangeKeys, type MileageRate, type RateVersion } from './api';
import { RatePerKmInput, RateSourceNote, TierInputs } from './RateInputs';
import {
  defaultRateName,
  emptyNewRateValues,
  fromRate,
  fromVersion,
  newRateSchema,
  newVersionValues,
  rateSchema,
  rateVersionSchema,
  toNewRateInput,
  toRateInput,
  toRateVersionInput,
  type NewRateValues,
  type RateValues,
  type RateVersionValues,
} from './rateForm';

function FormButtons({ label, loading, onCancel }: { label: string; loading: boolean; onCancel: () => void }) {
  return (
    <Group justify="flex-end">
      <Button variant="default" onClick={onCancel}>
        Cancel
      </Button>
      <Button type="submit" loading={loading}>
        {label}
      </Button>
    </Group>
  );
}

// ─── New rate ───────────────────────────────────────────────────────────────

function NewRateForm({ rates, onClose }: { rates: MileageRate[]; onClose: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<NewRateValues>({
    initialValues: emptyNewRateValues(business.current_financial_year.start, defaultRateName(rates)),
    validate: zodValidate<NewRateValues>(newRateSchema),
  });
  const save = useApiMutation(
    (values: NewRateValues) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/mileage/rates', {
          params: { path: { business_id: business.id } },
          body: toNewRateInput(values),
        }),
      ),
    { form, successMessage: (rate) => `${rate.name} added`, invalidate: mileageChangeKeys(business.id), onSuccess: onClose },
  );
  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <RateSourceNote />
        <TextInput label="Name" withAsterisk data-autofocus maxLength={80} {...form.getInputProps('name')} />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <RatePerKmInput currency={business.currency} withAsterisk {...form.getInputProps('rate_per_km')} />
          <DateInput
            label="Applies from"
            description="Trips from this date use it."
            withAsterisk
            valueFormat={dateInputFormat()}
            {...form.getInputProps('effective_from')}
          />
        </SimpleGrid>
        <TierInputs
          currency={business.currency}
          limitProps={form.getInputProps('tier1_limit_km')}
          rateProps={form.getInputProps('tier2_rate_per_km')}
        />
        <Textarea
          label="Notes"
          description="For your own reference, such as where the figure comes from."
          autosize
          minRows={2}
          maxRows={6}
          {...form.getInputProps('notes')}
        />
        <FormButtons label="Add rate" loading={save.isPending} onCancel={onClose} />
      </Stack>
    </form>
  );
}

/** Adds a mileage rate with its first version. `rates` are the existing ones (for the default name). */
export function NewRateModal({ opened, onClose, rates }: { opened: boolean; onClose: () => void; rates: MileageRate[] }) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title="Add mileage rate" fullScreen={isMobile}>
      <NewRateForm rates={rates} onClose={onClose} />
    </Modal>
  );
}

// ─── Name and notes ─────────────────────────────────────────────────────────

function RateDetailsForm({ rate, onClose }: { rate: MileageRate; onClose: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<RateValues>({ initialValues: fromRate(rate), validate: zodValidate<RateValues>(rateSchema) });
  const save = useApiMutation(
    (values: RateValues) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/mileage/rates/{rate_id}', {
          params: { path: { business_id: business.id, rate_id: rate.id } },
          body: toRateInput(values),
        }),
      ),
    { form, successMessage: (saved) => `${saved.name} saved`, invalidate: mileageChangeKeys(business.id), onSuccess: onClose },
  );
  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <TextInput label="Name" withAsterisk data-autofocus maxLength={80} {...form.getInputProps('name')} />
        <Textarea
          label="Notes"
          description="For your own reference, such as where the figure comes from."
          autosize
          minRows={2}
          maxRows={6}
          {...form.getInputProps('notes')}
        />
        <FormButtons label="Save changes" loading={save.isPending} onCancel={onClose} />
      </Stack>
    </form>
  );
}

/** Renames a rate or changes its notes. */
export function RateDetailsModal({ opened, onClose, rate }: { opened: boolean; onClose: () => void; rate: MileageRate | null }) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title="Edit mileage rate" fullScreen={isMobile}>
      {rate && <RateDetailsForm key={rate.id} rate={rate} onClose={onClose} />}
    </Modal>
  );
}

// ─── Versions ───────────────────────────────────────────────────────────────

function RateVersionForm({ rate, version, onClose }: { rate: MileageRate; version: RateVersion | null; onClose: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<RateVersionValues>({
    initialValues: version ? fromVersion(version) : newVersionValues(rate, business.current_financial_year.start, business.today),
    validate: zodValidate<RateVersionValues>(rateVersionSchema),
  });
  const save = useApiMutation(
    (values: RateVersionValues) => {
      const body = toRateVersionInput(values);
      return version
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/mileage/rates/{rate_id}/versions/{version_id}', {
              params: { path: { business_id: business.id, rate_id: rate.id, version_id: version.id } },
              body,
            }),
          )
        : unwrap(
            api.POST('/api/v1/businesses/{business_id}/mileage/rates/{rate_id}/versions', {
              params: { path: { business_id: business.id, rate_id: rate.id } },
              body,
            }),
          );
    },
    {
      form,
      successMessage: (saved) => `${saved.name} updated`,
      invalidate: mileageChangeKeys(business.id),
      onSuccess: onClose,
    },
  );
  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <Text size="sm">
          For{' '}
          <Text span fw={600}>
            {rate.name}
          </Text>
          . Trips on or after the date use this rate until the next version starts.
        </Text>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <DateInput
            label="Applies from"
            description="The first day of this rate."
            withAsterisk
            valueFormat={dateInputFormat()}
            {...form.getInputProps('effective_from')}
          />
          <RatePerKmInput currency={business.currency} withAsterisk data-autofocus {...form.getInputProps('rate_per_km')} />
        </SimpleGrid>
        <TierInputs
          currency={business.currency}
          limitProps={form.getInputProps('tier1_limit_km')}
          rateProps={form.getInputProps('tier2_rate_per_km')}
        />
        <Alert color="gray" variant="light" icon={<Info size={16} />}>
          Trips you’ve already recorded keep the rate they used, so this doesn’t change their claims.
        </Alert>
        <FormButtons label={version ? 'Save changes' : 'Add version'} loading={save.isPending} onCancel={onClose} />
      </Stack>
    </form>
  );
}

/** Adds a dated version to a rate (no `version`) or edits one. */
export function RateVersionModal({
  opened,
  onClose,
  rate,
  version,
}: {
  opened: boolean;
  onClose: () => void;
  rate: MileageRate | null;
  version: RateVersion | null;
}) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={version ? 'Edit rate version' : 'Add rate version'} fullScreen={isMobile}>
      {rate && <RateVersionForm key={`${rate.id}:${version?.id ?? 'new'}`} rate={rate} version={version} onClose={onClose} />}
    </Modal>
  );
}
