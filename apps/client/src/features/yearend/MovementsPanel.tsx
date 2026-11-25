import { Badge, Button, Group, Modal, Pagination, Paper, Select, SimpleGrid, Stack, Table, Text, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { ArrowLeftRight, Plus } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { MoneyInput, MoneyText } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate, formatMoney } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { PER_PAGE, useMovements, yearEndChangeKeys, type Movement, type MovementKind } from './api';
import { isInflow, movementBody, movementLabel, movementSchema, MOVEMENT_KINDS, type MovementValues } from './forms';

/** Records (or edits) money in or out. `preset` fills a new record, e.g. a GST payment for a return. */
export function MovementForm({
  movement,
  preset,
  onDone,
}: {
  movement: Movement | null;
  preset?: Partial<MovementValues>;
  onDone: () => void;
}) {
  const business = useCurrentBusiness();
  const form = useForm<MovementValues>({
    initialValues: movement
      ? {
          movement_date: movement.movement_date,
          kind: movement.kind as MovementKind,
          amount: movement.amount,
          description: movement.description,
          reference: movement.reference,
        }
      : { movement_date: business.today, kind: 'drawing', amount: null, description: '', reference: '', ...preset },
    validate: zodValidate<MovementValues>(movementSchema),
  });
  const invalidate = yearEndChangeKeys(business.id);
  const save = useApiMutation(
    (v: MovementValues) =>
      movement
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/cash-movements/{movement_id}', {
              params: { path: { business_id: business.id, movement_id: movement.id } },
              body: movementBody(v),
            }),
          )
        : unwrap(
            api.POST('/api/v1/businesses/{business_id}/cash-movements', {
              params: { path: { business_id: business.id } },
              body: movementBody(v),
            }),
          ),
    { form, invalidate, successMessage: (row) => `${movementLabel(row.kind)} ${movement ? 'saved' : 'recorded'}`, onSuccess: onDone },
  );
  const remove = useApiMutation(
    ({ id, reason }: { id: string; reason: string }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/cash-movements/{movement_id}', {
          params: { path: { business_id: business.id, movement_id: id } },
          body: { reason },
        }),
      ),
    { invalidate, successMessage: 'Record deleted', onSuccess: onDone },
  );
  const confirmDelete = async (row: Movement) => {
    const reason = await promptReason({
      title: 'Delete this record?',
      message: `The ${movementLabel(row.kind).toLowerCase()} of ${formatMoney(row.amount, row.currency)} will be removed from your reports. The record and your reason are kept in the history.`,
      confirmLabel: 'Delete',
      danger: true,
    });
    if (reason !== null) remove.mutate({ id: row.id, reason });
  };
  const kind = MOVEMENT_KINDS.find((k) => k.value === form.values.kind);
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <Select
            label="Type"
            data={MOVEMENT_KINDS.map(({ value, label }) => ({ value, label }))}
            description={kind?.hint}
            allowDeselect={false}
            {...form.getInputProps('kind')}
          />
          <DateInput label="Date" valueFormat={dateInputFormat()} {...form.getInputProps('movement_date')} />
          <MoneyInput
            label="Amount"
            currency={business.currency}
            value={form.values.amount}
            onChange={(v) => form.setFieldValue('amount', v)}
            error={form.errors.amount}
          />
          <TextInput label="Reference" placeholder="Bank reference" maxLength={100} {...form.getInputProps('reference')} />
        </SimpleGrid>
        <TextInput label="Description" maxLength={300} {...form.getInputProps('description')} />
        <Group justify="space-between">
          {movement ? (
            <Button variant="subtle" color="red" onClick={() => void confirmDelete(movement)} loading={remove.isPending}>
              Delete
            </Button>
          ) : (
            <span />
          )}
          <Group gap="sm">
            <Button variant="default" onClick={onDone}>
              Cancel
            </Button>
            <Button type="submit" loading={save.isPending}>
              {movement ? 'Save' : 'Record'}
            </Button>
          </Group>
        </Group>
      </Stack>
    </form>
  );
}

function Amount({ row }: { row: Movement }) {
  const inflow = isInflow(row.kind);
  return <MoneyText value={inflow ? row.amount : -row.amount} currency={row.currency} fw={500} c={inflow ? undefined : 'dimmed'} />;
}

function MovementRows({ rows, isMobile, onOpen }: { rows: Movement[]; isMobile: boolean; onOpen?: (row: Movement) => void }) {
  if (isMobile) {
    return (
      <Stack gap="xs">
        {rows.map((row) => (
          <Paper
            key={row.id}
            withBorder
            p="sm"
            radius="md"
            component={onOpen ? 'button' : 'div'}
            onClick={onOpen ? () => onOpen(row) : undefined}
            style={{ textAlign: 'left', width: '100%', cursor: onOpen ? 'pointer' : undefined }}
          >
            <Group justify="space-between" wrap="nowrap">
              <Stack gap={0} style={{ minWidth: 0 }}>
                <Text size="sm" fw={500}>
                  {movementLabel(row.kind)}
                </Text>
                <Text size="xs" c="dimmed" truncate="end">
                  {formatDate(row.movement_date)}
                  {row.description ? ` · ${row.description}` : ''}
                </Text>
              </Stack>
              <Amount row={row} />
            </Group>
          </Paper>
        ))}
      </Stack>
    );
  }
  return (
    <Table.ScrollContainer minWidth={560}>
      <Table highlightOnHover={!!onOpen} verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Date</Table.Th>
            <Table.Th>Type</Table.Th>
            <Table.Th>Description</Table.Th>
            <Table.Th ta="right">Amount</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((row) => (
            <Table.Tr key={row.id} onClick={onOpen ? () => onOpen(row) : undefined} style={{ cursor: onOpen ? 'pointer' : undefined }}>
              <Table.Td>{formatDate(row.movement_date)}</Table.Td>
              <Table.Td>
                <Badge variant="light" color={isInflow(row.kind) ? 'brand' : 'gray'}>
                  {movementLabel(row.kind)}
                </Badge>
              </Table.Td>
              <Table.Td>
                <Text size="sm" truncate="end" maw={320}>
                  {row.description || row.reference || '—'}
                </Text>
              </Table.Td>
              <Table.Td ta="right">
                <Amount row={row} />
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

/** Drawings, funds introduced and payments to or from IRD: money that isn't income or an expense. */
export function MovementsPanel() {
  const isMobile = useIsMobile();
  const [page, setPage] = useState(1);
  const [kind, setKind] = useState<MovementKind | null>(null);
  const [dialog, setDialog] = useState<{ opened: boolean; movement: Movement | null }>({ opened: false, movement: null });
  const list = useMovements(page, kind);
  const close = () => setDialog((d) => ({ ...d, opened: false }));
  const addButton = (
    <Button leftSection={<Plus size={16} />} onClick={() => setDialog({ opened: true, movement: null })}>
      Record money in or out
    </Button>
  );

  let content: ReactNode;
  if (list.isPending) content = <ListSkeleton rows={6} />;
  else if (list.isError) content = <ErrorState error={list.error} onRetry={() => void list.refetch()} />;
  else if (list.data.items.length === 0)
    content = (
      <EmptyBlock
        icon={ArrowLeftRight}
        title={kind ? `No ${movementLabel(kind).toLowerCase()} records` : 'Nothing recorded yet'}
        description="Record money you take out or put in, and GST and income tax you pay to or get back from IRD. Your balance sheet and statement of changes in equity use them."
        action={kind ? undefined : addButton}
      />
    );
  else {
    const { items, total } = list.data;
    content = (
      <Stack gap="md">
        <MovementRows rows={items} isMobile={isMobile} onOpen={(movement) => setDialog({ opened: true, movement })} />
        <Group justify="space-between">
          <Text size="sm" c="dimmed">
            {total === 1 ? '1 record' : `${total} records`}
          </Text>
          {total > PER_PAGE && <Pagination total={Math.ceil(total / PER_PAGE)} value={page} onChange={setPage} size="sm" />}
        </Group>
      </Stack>
    );
  }

  return (
    <SectionCard
      title="Money in and out"
      description="Owner drawings and contributions, and GST and income tax paid to or refunded by IRD."
      actions={!isMobile && addButton}
      padding={isMobile ? 'sm' : 'lg'}
    >
      <Stack gap="md">
        <Group justify="space-between" gap="sm">
          <Select
            aria-label="Filter by type"
            placeholder="All types"
            clearable
            data={MOVEMENT_KINDS.map(({ value, label }) => ({ value, label }))}
            value={kind}
            onChange={(value) => {
              setKind(value as MovementKind | null);
              setPage(1);
            }}
            w={isMobile ? '100%' : 240}
          />
          {isMobile && addButton}
        </Group>
        {content}
      </Stack>
      <Modal opened={dialog.opened} onClose={close} title={dialog.movement ? 'Edit money in or out' : 'Record money in or out'}>
        <MovementForm key={dialog.movement?.id ?? 'new'} movement={dialog.movement} onDone={close} />
      </Modal>
    </SectionCard>
  );
}
