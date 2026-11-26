import { Anchor, Button, Group, SimpleGrid, Stack, Switch, Text, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { MoneyText } from '@/components/Money';
import { SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { openExternal } from '@/services/platform';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { incomeTaxKeys, type AccSettings, type TaxYear } from './api';

const LEVY_GUIDE = 'https://www.acc.co.nz/for-business/received-an-invoice/calculate-your-levies';

function Line({ label, value, currency, strong }: { label: string; value: number; currency: string; strong?: boolean }) {
  return (
    <Group justify="space-between" wrap="nowrap" gap="sm">
      <Text size="sm" fw={strong ? 600 : undefined}>
        {label}
      </Text>
      <MoneyText value={value} currency={currency} size="sm" fw={strong ? 700 : undefined} style={{ flexShrink: 0 }} />
    </Group>
  );
}

function SettingsForm({ settings }: { settings: AccSettings }) {
  const business = useCurrentBusiness();
  const form = useForm({ initialValues: { work_levy_rate: settings.work_levy_rate ?? '', full_time: settings.full_time } });
  const save = useApiMutation(
    (v: typeof form.values) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/income-tax/acc', {
          params: { path: { business_id: business.id } },
          body: { work_levy_rate: v.work_levy_rate.trim() || null, full_time: v.full_time },
        }),
      ),
    { form, invalidate: [incomeTaxKeys.summary(business.id)], successMessage: 'ACC settings saved' },
  );
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="sm">
        <TextInput
          label="Work levy rate"
          description="Dollars per $100, excluding GST, for your industry. It’s on your ACC invoice."
          placeholder="e.g. 0.69"
          inputMode="decimal"
          leftSection="$"
          {...form.getInputProps('work_levy_rate')}
        />
        <Switch
          label="I work full-time"
          description="Over 30 hours a week on average: ACC levies at least its minimum earnings."
          {...form.getInputProps('full_time', { type: 'checkbox' })}
        />
        <Group justify="flex-end">
          <Button type="submit" size="xs" loading={save.isPending}>
            Save
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** The ACC levies on the selected year's profit, and the owner's work levy rate and full-time switch. */
export function AccLevies({ year, settings }: { year: TaxYear; settings: AccSettings }) {
  const business = useCurrentBusiness();
  const acc = year.acc;
  const currency = business.currency;
  return (
    <SectionCard
      title={`ACC levies, ${year.label}`}
      description="What ACC is likely to invoice for this year’s earnings once your income tax return is filed."
    >
      <SimpleGrid cols={{ base: 1, md: 2 }} spacing="lg">
        {acc ? (
          <Stack gap={6}>
            <Line label="Liable earnings" value={acc.liable_earnings} currency={currency} />
            {acc.adjustment !== 'none' && (
              <Text size="xs" c="dimmed">
                {acc.adjustment === 'minimum' ? (
                  <>
                    ACC’s minimum for full-time work (<MoneyText value={acc.min_liable} currency={currency} size="xs" />
                    ), as your profit is less.
                  </>
                ) : (
                  <>
                    ACC’s maximum (<MoneyText value={acc.max_liable} currency={currency} size="xs" />
                    ): earnings above it aren’t levied.
                  </>
                )}
              </Text>
            )}
            <Line label={`Earners’ levy ($${acc.earners_levy_rate} per $100)`} value={acc.earners_levy} currency={currency} />
            {acc.work_levy !== null && acc.work_levy !== undefined ? (
              <Line label={`Work levy ($${acc.work_levy_rate} per $100)`} value={acc.work_levy} currency={currency} />
            ) : (
              <Group justify="space-between" wrap="nowrap" gap="sm">
                <Text size="sm">Work levy</Text>
                <Text size="sm" c="dimmed">
                  Enter your rate
                </Text>
              </Group>
            )}
            <Line
              label={`Working Safer levy ($${acc.working_safer_levy_rate} per $100)`}
              value={acc.working_safer_levy}
              currency={currency}
            />
            <Line label="GST" value={acc.gst} currency={currency} />
            <Line label="ACC levies" value={acc.total} currency={currency} strong />
          </Stack>
        ) : (
          <Text size="sm" c="dimmed">
            ACC levies aren’t estimated for {year.label}: Solo has ACC’s rates from the 2024-25 year.
          </Text>
        )}
        <Stack gap="xs">
          <SettingsForm key={`${settings.work_levy_rate ?? ''}-${settings.full_time}`} settings={settings} />
          <Text size="xs" c="dimmed">
            Levies are per $100 of your profit, between ACC’s minimum and maximum earnings, plus GST. The work levy depends on your
            industry’s classification unit; find it on your ACC invoice or{' '}
            <Anchor
              size="xs"
              href={LEVY_GUIDE}
              onClick={(e) => {
                e.preventDefault();
                void openExternal(LEVY_GUIDE);
              }}
            >
              ACC’s levy information
            </Anchor>
            . ACC adds back levies you claimed as expenses, so the actual figure can be a little higher.
          </Text>
        </Stack>
      </SimpleGrid>
    </SectionCard>
  );
}
