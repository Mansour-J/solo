import { Button, Group, Modal, Select, SimpleGrid, Stack, Textarea, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { MoneyInput } from '@/components/Money';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { allocationChangeKeys, type Destination, type DestinationKind } from './api';
import {
  destinationSchema,
  emptyDestinationValues,
  fromDestination,
  toDestinationInput,
  type DestinationFormValues,
} from './destinationForm';
import { KIND_OPTIONS } from './format';

/** Add a destination (no `destination`) or edit one. */
export function DestinationFormModal({
  opened,
  onClose,
  destination,
}: {
  opened: boolean;
  onClose: () => void;
  destination?: Destination | null;
}) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={destination ? `Edit ${destination.name}` : 'Add destination'} fullScreen={isMobile}>
      <DestinationForm key={destination?.id ?? 'new'} destination={destination ?? null} onClose={onClose} />
    </Modal>
  );
}

function DestinationForm({ destination, onClose }: { destination: Destination | null; onClose: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<DestinationFormValues>({
    initialValues: destination ? fromDestination(destination) : emptyDestinationValues(),
    validate: zodValidate<DestinationFormValues>(destinationSchema),
  });
  const save = useApiMutation(
    (values: DestinationFormValues) => {
      const body = toDestinationInput(values);
      return destination
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/allocations/destinations/{destination_id}', {
              params: { path: { business_id: business.id, destination_id: destination.id } },
              body,
            }),
          )
        : unwrap(
            api.POST('/api/v1/businesses/{business_id}/allocations/destinations', { params: { path: { business_id: business.id } }, body }),
          );
    },
    {
      form,
      successMessage: (saved) => (destination ? `${saved.name} saved` : `${saved.name} added`),
      invalidate: allocationChangeKeys(business.id),
      onSuccess: onClose,
    },
  );

  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <TextInput
          label="Name"
          placeholder="e.g. Holiday fund"
          withAsterisk
          maxLength={80}
          data-autofocus
          {...form.getInputProps('name')}
        />
        <Select
          label="Type"
          description="Used to group and label it; it doesn’t change how amounts are worked out."
          data={KIND_OPTIONS}
          allowDeselect={false}
          value={form.values.kind}
          onChange={(value) => form.setFieldValue('kind', (value ?? 'custom') as DestinationKind)}
        />
        <SimpleGrid cols={{ base: 1, xs: 2 }}>
          <MoneyInput
            label="Target"
            description="Optional. Shows progress towards it."
            currency={business.currency}
            value={form.values.target_amount}
            onChange={(value) => form.setFieldValue('target_amount', value)}
            error={form.errors.target_amount}
          />
          <MoneyInput
            label="Minimum balance"
            description="Optional. Flags when the balance is below it."
            currency={business.currency}
            value={form.values.minimum_balance}
            onChange={(value) => form.setFieldValue('minimum_balance', value)}
            error={form.errors.minimum_balance}
          />
        </SimpleGrid>
        <Textarea label="Notes" autosize minRows={2} maxLength={1000} {...form.getInputProps('notes')} />
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {destination ? 'Save' : 'Add destination'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
