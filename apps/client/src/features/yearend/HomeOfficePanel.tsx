import { Alert, Button, Group, NumberInput, Select, SimpleGrid, Stack, Text, Textarea } from '@mantine/core';
import { useForm } from '@mantine/form';
import { Info } from 'lucide-react';
import { useState } from 'react';
import { z } from 'zod';
import { confirmAction } from '@/components/confirmAction';
import { MoneyInput, MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useHomeOffice, yearEndChangeKeys, type HomeOfficeYear } from './api';
import { recentYearEnds } from './forms';

interface HomeOfficeValues {
  business_area: number | string;
  total_area: number | string;
  premises_costs: number | null;
  notes: string;
}

const areaError = 'Enter the area in square metres.';
const homeOfficeSchema = z
  .object({
    business_area: z.coerce.number({ error: areaError }).positive(areaError),
    total_area: z.coerce.number({ error: areaError }).positive(areaError),
  })
  .passthrough();

function HomeOfficeForm({ year }: { year: HomeOfficeYear }) {
  const business = useCurrentBusiness();
  const claim = year.claim;
  const form = useForm<HomeOfficeValues>({
    initialValues: {
      business_area: claim ? Number(claim.business_area) : '',
      total_area: claim ? Number(claim.total_area) : '',
      premises_costs: claim ? claim.premises_costs : null,
      notes: claim?.notes ?? '',
    },
    validate: zodValidate<HomeOfficeValues>(homeOfficeSchema),
  });
  const invalidate = yearEndChangeKeys(business.id);
  const save = useApiMutation(
    (v: HomeOfficeValues) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/home-office', {
          params: { path: { business_id: business.id } },
          body: {
            year_end: year.year_end,
            business_area: String(v.business_area),
            total_area: String(v.total_area),
            premises_costs: v.premises_costs ?? 0,
            notes: v.notes,
          },
        }),
      ),
    { form, invalidate, successMessage: 'Home office claim saved' },
  );
  const remove = useApiMutation(
    () =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/home-office', {
          params: { path: { business_id: business.id }, query: { year_end: year.year_end } },
        }),
      ),
    { invalidate, successMessage: 'Home office claim removed' },
  );
  const confirmRemove = async () => {
    const ok = await confirmAction({
      title: 'Remove this year’s home office claim?',
      message: 'The home office expense for the year is taken out of your books.',
      confirmLabel: 'Remove',
      danger: true,
    });
    if (ok) remove.mutate();
  };

  if (year.rate === null || year.rate === undefined) {
    return (
      <Alert icon={<Info size={16} />} color="gray">
        IRD hasn’t published the square metre rate for this year yet. It usually does after the year ends.
      </Alert>
    );
  }
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack>
        <Text size="sm">
          IRD’s rate for the year: <MoneyText value={year.rate} currency={business.currency} size="sm" fw={600} /> a square metre, for
          power, gas, internet and similar costs.
        </Text>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <NumberInput
            label="Business area (m²)"
            description="Space set aside and used mainly for the business."
            min={0}
            decimalScale={2}
            {...form.getInputProps('business_area')}
          />
          <NumberInput
            label="Whole home (m²)"
            description="Your home’s total floor area."
            min={0}
            decimalScale={2}
            {...form.getInputProps('total_area')}
          />
          <MoneyInput
            label="Mortgage interest, rates and rent"
            description="Paid for the year; leave empty if none. Not mortgage principal."
            currency={business.currency}
            value={form.values.premises_costs}
            onChange={(v) => form.setFieldValue('premises_costs', v)}
            error={form.errors.premises_costs}
          />
        </SimpleGrid>
        <Textarea label="Notes" autosize minRows={2} {...form.getInputProps('notes')} />
        {claim && (
          <Group gap="xs">
            <Text size="sm" c="dimmed">
              Claimed for the year:
            </Text>
            <MoneyText value={claim.claim} currency={business.currency} fw={600} />
          </Group>
        )}
        <Group justify="space-between">
          {claim ? (
            <Button variant="subtle" color="red" onClick={() => void confirmRemove()} loading={remove.isPending}>
              Remove claim
            </Button>
          ) : (
            <span />
          )}
          <Button type="submit" loading={save.isPending}>
            {claim ? 'Save changes' : 'Claim home office'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** Year end › Home office: a yearly claim by IRD's square metre rate option. */
export function HomeOfficePanel() {
  const business = useCurrentBusiness();
  const years = recentYearEnds(business.current_financial_year.end);
  const [yearEnd, setYearEnd] = useState(years[1] ?? years[0]!);
  const year = useHomeOffice(yearEnd);

  let content;
  if (year.isPending) content = <ListSkeleton rows={3} height={44} />;
  else if (year.isError) content = <ErrorState error={year.error} onRetry={() => void year.refetch()} />;
  else content = <HomeOfficeForm key={`${yearEnd}-${year.data.claim?.updated_at ?? 'new'}`} year={year.data} />;

  return (
    <SectionCard
      title="Home office"
      description="Claim the business use of your home with IRD’s square metre rate: the business share of mortgage interest, rates and rent, plus the business area times IRD’s rate. If you use it, don’t also claim your home’s power or internet as expenses."
      actions={
        <Select
          aria-label="Financial year ending"
          size="xs"
          w={170}
          allowDeselect={false}
          data={years.map((y) => ({ value: y, label: `Year to ${formatDate(y)}` }))}
          value={yearEnd}
          onChange={(v) => v && setYearEnd(v)}
        />
      }
    >
      {content}
    </SectionCard>
  );
}
