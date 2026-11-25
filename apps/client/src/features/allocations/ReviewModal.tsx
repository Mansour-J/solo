import { ActionIcon, Alert, Anchor, Badge, Box, Button, Group, Modal, Paper, Select, Stack, Table, Text } from '@mantine/core';
import { Info, Plus, X } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { BRAND } from '@/branding/brand';
import { MoneyInput, MoneyText } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate, formatMoney } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { isApiError } from '@/services/api/errors';
import { useApiMutation } from '@/services/api/query';
import {
  allocationChangeKeys,
  useAllocation,
  useDestinations,
  useReviewCalculation,
  type Allocation,
  type Destination,
  type LineInput,
  type PlanLine,
} from './api';
import { limitLabel, sourceText, statusOf } from './format';

/** Review an allocation: confirm (or adjust) a pending one, or see and reverse a confirmed one. */
export function ReviewModal({ allocationId, onClose }: { allocationId: string | null; onClose: () => void }) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={allocationId !== null} onClose={onClose} title="Allocation" size="lg" fullScreen={isMobile}>
      {allocationId && <ReviewLoader key={allocationId} allocationId={allocationId} onClose={onClose} />}
    </Modal>
  );
}

function ReviewLoader({ allocationId, onClose }: { allocationId: string; onClose: () => void }) {
  const allocation = useAllocation(allocationId);
  const destinations = useDestinations();
  if (allocation.isPending || destinations.isPending) return <ListSkeleton rows={5} height={44} />;
  if (allocation.isError) return <ErrorState error={allocation.error} onRetry={() => void allocation.refetch()} />;
  if (destinations.isError) return <ErrorState error={destinations.error} onRetry={() => void destinations.refetch()} />;
  const open = allocation.data.status === 'pending' || allocation.data.status === 'skipped';
  return open ? (
    <ReviewForm key={allocation.data.id} allocation={allocation.data} destinations={destinations.data} onClose={onClose} />
  ) : (
    <Recorded allocation={allocation.data} onClose={onClose} />
  );
}

function ReceiptSummary({ allocation }: { allocation: Allocation }) {
  const status = statusOf(allocation.status);
  const sourcePath =
    allocation.source_type === 'invoice_payment' && allocation.invoice_id ? `/invoices/${allocation.invoice_id}` : '/income';
  return (
    <Paper withBorder radius="md" p="md" bg="var(--app-surface-raised)">
      <Group justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
        <Box style={{ minWidth: 0 }}>
          <Text size="xs" c="dimmed">
            {sourceText(allocation.source_type)} · {formatDate(allocation.received_date)}
          </Text>
          <Anchor component={Link} to={sourcePath} size="sm" fw={500} className="clamp-1">
            {allocation.source_label}
          </Anchor>
          {allocation.customer_name && (
            <Text size="xs" c="dimmed" className="clamp-1">
              {allocation.customer_name}
            </Text>
          )}
        </Box>
        <Stack gap={4} align="flex-end" style={{ flexShrink: 0 }}>
          <Text size="xs" c="dimmed">
            Amount received
          </Text>
          <MoneyText value={allocation.received_amount} currency={allocation.currency} fw={650} size="lg" />
          <Badge variant="light" color={status.color}>
            {status.label}
          </Badge>
        </Stack>
      </Group>
    </Paper>
  );
}

const PLANNING_NOTE = `This records your plan for the money. ${BRAND.name} doesn’t move money between accounts.`;

interface EditLine {
  destination_id: string;
  rule_id: string | null;
  /** Minor units; null while what's typed isn't an amount. */
  amount: number | null;
  /** The rule's plan, kept to explain the amount. */
  planned?: PlanLine;
}

function ReviewForm({ allocation, destinations, onClose }: { allocation: Allocation; destinations: Destination[]; onClose: () => void }) {
  const business = useCurrentBusiness();
  const currency = allocation.currency;
  const [lines, setLines] = useState<EditLine[]>(() =>
    (allocation.plan?.lines ?? []).map((l) => ({
      destination_id: l.destination_id,
      rule_id: l.rule_id ?? null,
      amount: l.amount,
      planned: l,
    })),
  );
  const input: LineInput[] | null = lines.some((l) => l.amount === null)
    ? null
    : lines.map((l) => ({ destination_id: l.destination_id, amount: l.amount ?? 0, rule_id: l.rule_id }));
  const { calculation, error } = useReviewCalculation(allocation.id, input);
  const fieldErrors = isApiError(error) ? error.fields : {};
  const after = new Map((calculation?.lines ?? []).map((l) => [l.destination_id, l]));
  const allocated = calculation?.allocated ?? allocation.plan?.allocated ?? 0;
  const remaining = calculation?.remaining ?? allocation.plan?.remaining ?? allocation.received_amount;

  const path = { business_id: business.id, allocation_id: allocation.id };
  const confirm = useApiMutation(
    (body: LineInput[]) =>
      unwrap(api.POST('/api/v1/businesses/{business_id}/allocations/{allocation_id}/confirm', { params: { path }, body: { lines: body } })),
    { successMessage: 'Allocation recorded', invalidate: allocationChangeKeys(business.id), onSuccess: onClose },
  );
  const skip = useApiMutation(
    () => unwrap(api.POST('/api/v1/businesses/{business_id}/allocations/{allocation_id}/skip', { params: { path } })),
    { successMessage: 'Allocation skipped', invalidate: allocationChangeKeys(business.id), onSuccess: onClose },
  );
  const confirmErrors = isApiError(confirm.error) ? confirm.error.fields : {};
  const errorFor = (i: number, field: string) => confirmErrors[`lines.${i}.${field}`] ?? fieldErrors[`lines.${i}.${field}`];
  const totalError = confirmErrors.lines ?? fieldErrors.lines;

  const used = new Set(lines.map((l) => l.destination_id));
  const addOptions = destinations.filter((d) => !d.archived && !used.has(d.id)).map((d) => ({ value: d.id, label: d.name }));
  const nameOf = (id: string) => destinations.find((d) => d.id === id)?.name ?? '';
  const update = (index: number, amount: number | null) =>
    setLines((current) => current.map((l, i) => (i === index ? { ...l, amount } : l)));

  return (
    <Stack gap="md">
      <ReceiptSummary allocation={allocation} />
      {allocation.status === 'skipped' && (
        <Alert color="gray" icon={<Info size={16} />}>
          You skipped this allocation. You can still record it.
        </Alert>
      )}
      {lines.length === 0 ? (
        <Text size="sm" c="dimmed">
          None of your rules apply to this money. Add a destination below to set some aside, or skip it.
        </Text>
      ) : (
        <Stack gap="xs">
          {lines.map((line, i) => {
            const planned = line.planned;
            const changed = planned && line.amount !== planned.amount;
            const hint = changed ? `Proposed ${formatMoney(planned.amount, currency)}` : limitLabel(planned?.limited_by ?? null);
            const balance = after.get(line.destination_id);
            return (
              <Paper key={line.destination_id} withBorder radius="md" p="sm">
                <Group justify="space-between" align="flex-start" wrap="wrap" gap="sm">
                  <Box style={{ minWidth: 0, flex: '1 1 180px' }}>
                    <Text size="sm" fw={500}>
                      {nameOf(line.destination_id)}
                    </Text>
                    {hint && (
                      <Text size="xs" c="dimmed">
                        {hint}
                      </Text>
                    )}
                    {balance && (
                      <Text size="xs" c="dimmed">
                        Balance after: {formatMoney(balance.balance_after, currency)}
                        {balance.target_amount ? ` of ${formatMoney(balance.target_amount, currency)} target` : ''}
                      </Text>
                    )}
                  </Box>
                  <Group gap={6} wrap="nowrap" align="flex-start">
                    <MoneyInput
                      aria-label={`Amount for ${nameOf(line.destination_id)}`}
                      currency={currency}
                      value={line.amount}
                      onChange={(value) => update(i, value)}
                      error={errorFor(i, 'amount') ?? errorFor(i, 'destination_id')}
                      w={150}
                    />
                    {!planned && (
                      <ActionIcon
                        variant="subtle"
                        color="gray"
                        mt={4}
                        aria-label={`Remove ${nameOf(line.destination_id)}`}
                        onClick={() => setLines((current) => current.filter((_, j) => j !== i))}
                      >
                        <X size={16} />
                      </ActionIcon>
                    )}
                  </Group>
                </Group>
              </Paper>
            );
          })}
        </Stack>
      )}
      {addOptions.length > 0 && (
        <Select
          aria-label="Add a destination"
          placeholder="Add a destination"
          leftSection={<Plus size={15} />}
          data={addOptions}
          value={null}
          onChange={(id) => id && setLines((current) => [...current, { destination_id: id, rule_id: null, amount: 0 }])}
          w={{ base: '100%', xs: 260 }}
        />
      )}
      <Table withRowBorders={false} verticalSpacing={4}>
        <Table.Tbody>
          <Table.Tr>
            <Table.Td px={0}>Allocated</Table.Td>
            <Table.Td px={0} ta="right">
              <MoneyText value={allocated} currency={currency} size="sm" />
            </Table.Td>
          </Table.Tr>
          <Table.Tr>
            <Table.Td px={0} fw={600}>
              Remaining
            </Table.Td>
            <Table.Td px={0} ta="right">
              <MoneyText value={remaining} currency={currency} fw={650} />
            </Table.Td>
          </Table.Tr>
        </Table.Tbody>
      </Table>
      {totalError && (
        <Text size="sm" c="red" role="alert">
          {totalError}
        </Text>
      )}
      <Text size="xs" c="dimmed">
        {PLANNING_NOTE}
      </Text>
      <Group justify="space-between" gap="xs">
        {allocation.status === 'pending' ? (
          <Button variant="subtle" color="gray" loading={skip.isPending} onClick={() => skip.mutate(undefined)}>
            Skip
          </Button>
        ) : (
          <span />
        )}
        <Group gap="xs">
          <Button variant="default" onClick={onClose}>
            Later
          </Button>
          <Button loading={confirm.isPending} disabled={input === null} onClick={() => input && confirm.mutate(input)}>
            Confirm allocation
          </Button>
        </Group>
      </Group>
    </Stack>
  );
}

function Recorded({ allocation, onClose }: { allocation: Allocation; onClose: () => void }) {
  const business = useCurrentBusiness();
  const currency = allocation.currency;
  const reverse = useApiMutation(
    (reason: string) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/{allocation_id}/reverse', {
          params: { path: { business_id: business.id, allocation_id: allocation.id } },
          body: { reason },
        }),
      ),
    {
      successMessage: 'Allocation reversed. Review it again when you’re ready.',
      invalidate: allocationChangeKeys(business.id),
      onSuccess: onClose,
    },
  );
  const reverseIt = async () => {
    const reason = await promptReason({
      title: 'Reverse this allocation?',
      message: 'Its amounts come off the balances, and it goes back to your list to review again. The history keeps both.',
      label: 'Reason',
      confirmLabel: 'Reverse',
    });
    if (reason !== null) reverse.mutate(reason);
  };
  return (
    <Stack gap="md">
      <ReceiptSummary allocation={allocation} />
      {allocation.close_reason && (
        <Text size="sm" c="dimmed">
          {allocation.status === 'cancelled' ? 'Cancelled' : 'Reversed'}: {allocation.close_reason}
        </Text>
      )}
      {allocation.entries.length === 0 ? (
        <Text size="sm" c="dimmed">
          Nothing was set aside from this money.
        </Text>
      ) : (
        <Table.ScrollContainer minWidth={360}>
          <Table>
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Destination</Table.Th>
                <Table.Th>Recorded</Table.Th>
                <Table.Th ta="right">Amount</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {allocation.entries.map((e) => (
                <Table.Tr key={e.id}>
                  <Table.Td>
                    {e.destination_name}
                    {e.kind === 'reversal' && (
                      <Text span size="xs" c="dimmed">
                        {' '}
                        (reversal)
                      </Text>
                    )}
                  </Table.Td>
                  <Table.Td>{formatDate(e.created_at)}</Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={e.amount} currency={currency} size="sm" />
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Table.ScrollContainer>
      )}
      <Text size="xs" c="dimmed">
        {PLANNING_NOTE}
      </Text>
      <Group justify="flex-end" gap="xs">
        {allocation.status === 'confirmed' && (
          <Button variant="default" color="red" loading={reverse.isPending} onClick={() => void reverseIt()}>
            Reverse
          </Button>
        )}
        <Button variant="default" onClick={onClose}>
          Close
        </Button>
      </Group>
    </Stack>
  );
}
