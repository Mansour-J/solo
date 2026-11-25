import { ActionIcon, Button, Group, Modal, Select, SimpleGrid, Stack, Table, Text, Textarea, TextInput, Tooltip } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { Plus, Trash2, Wallet } from 'lucide-react';
import { useState } from 'react';
import { z } from 'zod';
import { MoneyInput, MoneyText } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { allocationChangeKeys, useAllocationBalances, useSpending, type Spending } from './api';

interface SpendingValues {
  destination_id: string | null;
  spent_on: string | null;
  amount: number | null;
  description: string;
  notes: string;
}

const spendingSchema = z
  .object({
    destination_id: z
      .string()
      .nullable()
      .refine((v) => !!v, 'Choose where the money came from.'),
    spent_on: z
      .string()
      .nullable()
      .refine((v) => !!v, 'Enter the date.'),
    amount: z
      .number()
      .nullable()
      .refine((v) => v !== null && v > 0, 'Enter the amount spent.'),
    description: z.string().trim().min(1, 'Say what it was spent on.').max(200, 'Use 200 characters or fewer.'),
  })
  .loose();

function SpendingForm({ onClose }: { onClose: () => void }) {
  const business = useCurrentBusiness();
  const balances = useAllocationBalances();
  const form = useForm<SpendingValues>({
    initialValues: { destination_id: null, spent_on: business.today, amount: null, description: '', notes: '' },
    validate: zodValidate<SpendingValues>(spendingSchema),
  });
  const save = useApiMutation(
    (v: SpendingValues) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/spending', {
          params: { path: { business_id: business.id } },
          body: {
            destination_id: v.destination_id ?? '',
            spent_on: v.spent_on ?? '',
            amount: v.amount ?? 0,
            description: v.description.trim(),
            notes: v.notes.trim(),
          },
        }),
      ),
    { form, invalidate: allocationChangeKeys(business.id), successMessage: 'Spending recorded', onSuccess: onClose },
  );
  const options = (balances.data?.destinations ?? [])
    .filter((d) => d.balance > 0)
    .map((d) => ({ value: d.id, label: d.name, balance: d.balance }));
  const chosen = options.find((o) => o.value === form.values.destination_id);
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="md">
        <Text size="sm">
          Money you used from what you’d set aside, such as a tax payment from your income tax savings. It’s a record only: nothing is paid
          or moved.
        </Text>
        <Select
          label="Spent from"
          withAsterisk
          data-autofocus
          placeholder={options.length ? 'Choose a destination' : 'Nothing is set aside yet'}
          data={options.map(({ value, label }) => ({ value, label }))}
          {...form.getInputProps('destination_id')}
          description={chosen ? <MoneyText value={chosen.balance} currency={business.currency} size="xs" inherit /> : undefined}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <DateInput label="Date" withAsterisk valueFormat={dateInputFormat()} {...form.getInputProps('spent_on')} />
          <MoneyInput
            label="Amount"
            withAsterisk
            currency={business.currency}
            value={form.values.amount}
            onChange={(v) => form.setFieldValue('amount', v)}
            error={form.errors.amount}
          />
        </SimpleGrid>
        <TextInput
          label="What for"
          withAsterisk
          maxLength={200}
          placeholder="e.g. Provisional tax, 2nd instalment"
          {...form.getInputProps('description')}
        />
        <Textarea label="Notes" autosize minRows={2} maxLength={1000} {...form.getInputProps('notes')} />
        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            Record spending
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

function SpendingTable({ rows }: { rows: Spending[] }) {
  const business = useCurrentBusiness();
  const remove = useApiMutation(
    ({ id, reason }: { id: string; reason: string }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/allocations/spending/{spending_id}', {
          params: { path: { business_id: business.id, spending_id: id } },
          body: { reason },
        }),
      ),
    { invalidate: allocationChangeKeys(business.id), successMessage: 'Spending deleted' },
  );
  const confirmRemove = async (row: Spending) => {
    const reason = await promptReason({
      title: 'Delete this spending?',
      message: `${row.description} goes back into ${row.destination_name}. It stays in the history with your reason.`,
      confirmLabel: 'Delete',
    });
    if (reason !== null) remove.mutate({ id: row.id, reason });
  };
  return (
    <Table.ScrollContainer minWidth={560}>
      <Table verticalSpacing="xs">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Date</Table.Th>
            <Table.Th>Spent from</Table.Th>
            <Table.Th>What for</Table.Th>
            <Table.Th ta="right">Amount</Table.Th>
            <Table.Th />
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((row) => (
            <Table.Tr key={row.id}>
              <Table.Td>{formatDate(row.spent_on)}</Table.Td>
              <Table.Td>{row.destination_name}</Table.Td>
              <Table.Td>{row.description}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={row.amount} currency={business.currency} size="sm" />
              </Table.Td>
              <Table.Td ta="right">
                <Tooltip label="Delete">
                  <ActionIcon variant="subtle" color="red" aria-label={`Delete ${row.description}`} onClick={() => void confirmRemove(row)}>
                    <Trash2 size={15} />
                  </ActionIcon>
                </Tooltip>
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

/** Allocations › Spending: money used from what was set aside. */
export function SpendingPanel() {
  const isMobile = useIsMobile();
  const spending = useSpending();
  const [opened, setOpened] = useState(false);
  const add = (
    <Button leftSection={<Plus size={16} />} onClick={() => setOpened(true)}>
      Record spending
    </Button>
  );
  let content;
  if (spending.isPending) content = <ListSkeleton rows={3} />;
  else if (spending.isError) content = <ErrorState error={spending.error} onRetry={() => void spending.refetch()} />;
  else if (spending.data.length === 0)
    content = (
      <EmptyBlock
        icon={Wallet}
        title="No spending recorded"
        description="When you use money you’d set aside, such as paying IRD from your GST savings, record it here so the balance stays true."
        action={add}
      />
    );
  else content = <SpendingTable rows={spending.data} />;
  return (
    <SectionCard
      title="Spending"
      description="Money used from what you’d set aside. Recorded only: nothing is paid or moved."
      actions={spending.data?.length ? add : undefined}
    >
      {content}
      <Modal opened={opened} onClose={() => setOpened(false)} title="Record spending" fullScreen={isMobile}>
        {opened && <SpendingForm onClose={() => setOpened(false)} />}
      </Modal>
    </SectionCard>
  );
}

/** Opens allocations for money received in a period before any rule was active. */
export function BackfillButton() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const [opened, setOpened] = useState(false);
  const [from, setFrom] = useState<string | null>(business.current_financial_year.start);
  const [to, setTo] = useState<string | null>(business.today);
  const run = useApiMutation(
    () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/backfill', {
          params: { path: { business_id: business.id } },
          body: { from: from ?? '', to: to ?? '' },
        }),
      ),
    {
      invalidate: allocationChangeKeys(business.id),
      successMessage: (r) =>
        r.opened === 0
          ? 'Every payment in that period already has an allocation'
          : `${r.opened} ${r.opened === 1 ? 'payment' : 'payments'} to review`,
      onSuccess: () => setOpened(false),
    },
  );
  return (
    <>
      <Button size="xs" variant="default" onClick={() => setOpened(true)}>
        Allocate earlier payments
      </Button>
      <Modal opened={opened} onClose={() => setOpened(false)} title="Allocate earlier payments" fullScreen={isMobile}>
        <Stack gap="md">
          <Text size="sm">
            Payments and income you recorded before you had rules weren’t allocated. Choose a period: each one in it gets a proposed split
            to review, once.
          </Text>
          <SimpleGrid cols={2}>
            <DateInput label="From" valueFormat={dateInputFormat()} value={from} onChange={setFrom} />
            <DateInput label="To" valueFormat={dateInputFormat()} value={to} onChange={setTo} />
          </SimpleGrid>
          <Group justify="flex-end">
            <Button variant="default" onClick={() => setOpened(false)}>
              Cancel
            </Button>
            <Button onClick={() => run.mutate()} loading={run.isPending} disabled={!from || !to}>
              Find payments
            </Button>
          </Group>
        </Stack>
      </Modal>
    </>
  );
}
