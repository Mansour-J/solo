import { Alert, Button, Group, SimpleGrid, Stack, Text } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { Info } from 'lucide-react';
import { confirmAction } from '@/components/confirmAction';
import { MoneyInput, MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useOpening, yearEndChangeKeys, type OpeningView } from './api';
import { openingBody, openingSchema, type OpeningValues } from './forms';

const FIELDS: { name: keyof Omit<OpeningValues, 'as_at'>; label: string; description: string; allowNegative?: boolean }[] = [
  { name: 'bank', label: 'Business bank account', description: 'The balance on the date. Negative for an overdraft.', allowNegative: true },
  {
    name: 'gst_payable',
    label: 'GST payable',
    description: 'GST owed to IRD on the date. Negative for a refund due.',
    allowNegative: true,
  },
  { name: 'drawings', label: 'Drawings to date', description: 'Total you’ve taken out of the business, as on the balance sheet.' },
  { name: 'funds_introduced', label: 'Funds introduced to date', description: 'Total of your own money you’ve put in.' },
  {
    name: 'personal_accounts',
    label: 'Personal accounts',
    description: 'Any other owner balance on the balance sheet.',
    allowNegative: true,
  },
  { name: 'withholding_tax', label: 'Withholding tax paid to date', description: 'Tax customers withheld from your payments.' },
  { name: 'income_tax', label: 'Income tax paid to date', description: 'Income tax you paid from the business.' },
];

function OpeningForm({ view }: { view: OpeningView }) {
  const business = useCurrentBusiness();
  const b = view.balances;
  const form = useForm<OpeningValues>({
    initialValues: {
      as_at: b?.as_at ?? '',
      bank: b?.bank ?? null,
      gst_payable: b?.gst_payable ?? null,
      drawings: b ? Math.abs(b.drawings) : null,
      funds_introduced: b?.funds_introduced ?? null,
      personal_accounts: b?.personal_accounts ?? null,
      withholding_tax: b ? Math.abs(b.withholding_tax) : null,
      income_tax: b ? Math.abs(b.income_tax) : null,
    },
    validate: zodValidate<OpeningValues>(openingSchema),
  });
  const invalidate = yearEndChangeKeys(business.id);
  const save = useApiMutation(
    (v: OpeningValues) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/opening-balances', {
          params: { path: { business_id: business.id } },
          body: openingBody(v),
        }),
      ),
    { form, invalidate, successMessage: 'Opening balances saved' },
  );
  const remove = useApiMutation(
    () => unwrap(api.DELETE('/api/v1/businesses/{business_id}/opening-balances', { params: { path: { business_id: business.id } } })),
    { invalidate, successMessage: 'Opening balances removed', onSuccess: () => form.reset() },
  );
  const confirmRemove = async () => {
    const ok = await confirmAction({
      title: 'Remove the opening balances?',
      message: 'Your balance sheet will start from zero. Assets brought forward keep their values but are no longer posted.',
      confirmLabel: 'Remove',
      danger: true,
    });
    if (ok) remove.mutate();
  };
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack>
        <DateInput
          label="As at"
          description="The last day before your records in the app start, usually the end of the financial year your accountant last reported on."
          valueFormat={dateInputFormat()}
          maw={320}
          {...form.getInputProps('as_at')}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          {FIELDS.map((f) => (
            <MoneyInput
              key={f.name}
              label={f.label}
              description={f.description}
              currency={business.currency}
              allowNegative={f.allowNegative}
              value={form.values[f.name]}
              onChange={(v) => form.setFieldValue(f.name, v)}
              error={form.errors[f.name]}
            />
          ))}
        </SimpleGrid>
        <Group gap="lg">
          <Stack gap={0}>
            <Text size="xs" c="dimmed">
              Assets brought forward
            </Text>
            <MoneyText value={view.fixed_assets} fw={500} />
          </Stack>
          {view.retained_earnings !== null && view.retained_earnings !== undefined && (
            <Stack gap={0}>
              <Text size="xs" c="dimmed">
                Retained earnings (balancing figure)
              </Text>
              <MoneyText value={view.retained_earnings} fw={500} />
            </Stack>
          )}
        </Group>
        {
          <Group justify="space-between">
            {b ? (
              <Button variant="subtle" color="red" onClick={() => void confirmRemove()} loading={remove.isPending}>
                Remove
              </Button>
            ) : (
              <span />
            )}
            <Button type="submit" loading={save.isPending}>
              Save opening balances
            </Button>
          </Group>
        }
      </Stack>
    </form>
  );
}

/** Balances brought forward from the accountant's last balance sheet, so this year's balance sheet is complete. */
export function OpeningPanel() {
  const isMobile = useIsMobile();
  const opening = useOpening();
  return (
    <SectionCard
      title="Opening balances"
      description="Copy these from your accountant’s last balance sheet. Assets go on the Assets tab with their value at this date."
      padding={isMobile ? 'sm' : 'lg'}
    >
      {opening.isPending ? (
        <ListSkeleton rows={4} />
      ) : opening.isError ? (
        <ErrorState error={opening.error} onRetry={() => void opening.refetch()} />
      ) : (
        <Stack>
          {opening.data.balances && (
            <Alert variant="light" icon={<Info size={16} />}>
              Posted to your books as at {formatDate(opening.data.balances.as_at)}.
            </Alert>
          )}
          <OpeningForm key={opening.data.balances?.updated_at ?? 'new'} view={opening.data} />
        </Stack>
      )}
    </SectionCard>
  );
}
