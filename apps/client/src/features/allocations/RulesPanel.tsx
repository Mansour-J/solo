import { ActionIcon, Badge, Box, Button, Group, Paper, Select, SimpleGrid, Stack, Table, Text, Tooltip } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { keepPreviousData, useQuery } from '@tanstack/react-query';
import { ArrowDown, ArrowUp, Archive, ListOrdered, Pencil, Plus, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { MoneyInput, MoneyText } from '@/components/Money';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { allocationChangeKeys, useRules, type Rule } from './api';
import { limitLabel, ruleAmountText, ruleDetails } from './format';
import { RuleFormModal } from './RuleFormModal';

function RuleCard({
  rule,
  index,
  count,
  currency,
  onEdit,
  onMove,
  onArchive,
  busy,
}: {
  rule: Rule;
  index: number;
  count: number;
  currency: string;
  onEdit: () => void;
  onMove: (delta: number) => void;
  onArchive: (archived: boolean) => void;
  busy: boolean;
}) {
  const details = ruleDetails(rule, currency);
  return (
    <Paper withBorder radius="md" p="sm">
      <Group justify="space-between" wrap="nowrap" gap="sm" align="flex-start">
        <Group gap="sm" wrap="nowrap" align="flex-start" style={{ minWidth: 0 }}>
          {!rule.archived && (
            <Badge variant="light" color="gray" circle aria-label={`Applies ${index + 1} of ${count}`}>
              {index + 1}
            </Badge>
          )}
          <Box style={{ minWidth: 0 }}>
            <Text size="sm" fw={500}>
              {ruleAmountText(rule, currency)} → {rule.destination_name}
            </Text>
            {details.length > 0 && (
              <Text size="xs" c="dimmed">
                {details.join(' · ')}
              </Text>
            )}
          </Box>
        </Group>
        {
          <Group gap={2} wrap="nowrap" style={{ flexShrink: 0 }}>
            {!rule.archived && (
              <>
                <ActionIcon
                  variant="subtle"
                  color="gray"
                  aria-label="Apply earlier"
                  disabled={index === 0 || busy}
                  onClick={() => onMove(-1)}
                >
                  <ArrowUp size={16} />
                </ActionIcon>
                <ActionIcon
                  variant="subtle"
                  color="gray"
                  aria-label="Apply later"
                  disabled={index === count - 1 || busy}
                  onClick={() => onMove(1)}
                >
                  <ArrowDown size={16} />
                </ActionIcon>
                <ActionIcon variant="subtle" color="gray" aria-label="Edit rule" onClick={onEdit}>
                  <Pencil size={16} />
                </ActionIcon>
              </>
            )}
            <Tooltip label={rule.archived ? 'Restore' : 'Archive'}>
              <ActionIcon
                variant="subtle"
                color="gray"
                aria-label={rule.archived ? 'Restore rule' : 'Archive rule'}
                disabled={busy}
                onClick={() => onArchive(!rule.archived)}
              >
                {rule.archived ? <RotateCcw size={16} /> : <Archive size={16} />}
              </ActionIcon>
            </Tooltip>
          </Group>
        }
      </Group>
    </Paper>
  );
}

/** Tries the active rules on an amount (the API plans it; nothing is recorded). */
function TryRules({ currency, today }: { currency: string; today: string }) {
  const business = useCurrentBusiness();
  const [amount, setAmount] = useState<number | null>(100_000);
  const [source, setSource] = useState<'invoice_payment' | 'income'>('invoice_payment');
  const [debounced] = useDebouncedValue({ amount, source }, 300);
  const preview = useQuery({
    queryKey: ['business', business.id, 'allocations', 'preview', debounced, today],
    queryFn: () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/preview', {
          params: { path: { business_id: business.id } },
          body: { amount: debounced.amount ?? 0, date: today, source: debounced.source },
        }),
      ),
    enabled: (debounced.amount ?? 0) > 0,
    placeholderData: keepPreviousData,
    retry: false,
  });
  const plan = (amount ?? 0) > 0 ? preview.data : undefined;
  return (
    <SectionCard title="Try your rules" description="See how an amount received today would be split. Nothing is recorded.">
      <Stack gap="md">
        <SimpleGrid cols={{ base: 1, xs: 2 }}>
          <MoneyInput label="Amount received" currency={currency} value={amount} onChange={setAmount} />
          <Select
            label="Received as"
            allowDeselect={false}
            data={[
              { value: 'invoice_payment', label: 'An invoice payment' },
              { value: 'income', label: 'Income without an invoice' },
            ]}
            value={source}
            onChange={(value) => setSource((value ?? 'invoice_payment') as typeof source)}
          />
        </SimpleGrid>
        {preview.isError ? (
          <ErrorState error={preview.error} onRetry={() => void preview.refetch()} />
        ) : plan ? (
          <Table withRowBorders={false} verticalSpacing={4} style={{ opacity: preview.isPlaceholderData ? 0.6 : 1 }}>
            <Table.Tbody>
              {plan.lines.map((l, i) => (
                <Table.Tr key={`${l.destination_id}-${i}`}>
                  <Table.Td px={0}>
                    <Text size="sm">{l.destination_name}</Text>
                    {limitLabel(l.limited_by ?? null) && (
                      <Text size="xs" c="dimmed">
                        {limitLabel(l.limited_by ?? null)}
                      </Text>
                    )}
                  </Table.Td>
                  <Table.Td px={0} ta="right">
                    <MoneyText value={l.amount} currency={currency} size="sm" />
                  </Table.Td>
                </Table.Tr>
              ))}
              <Table.Tr>
                <Table.Td px={0} fw={600}>
                  Remaining
                </Table.Td>
                <Table.Td px={0} ta="right">
                  <MoneyText value={plan.remaining} currency={currency} fw={650} />
                </Table.Td>
              </Table.Tr>
            </Table.Tbody>
          </Table>
        ) : (
          <Text size="sm" c="dimmed">
            Enter an amount to see the split.
          </Text>
        )}
      </Stack>
    </SectionCard>
  );
}

/** Rules in the order they apply, with reordering, editing and archiving, and a preview. */
export function RulesPanel({ creating, onCloseCreate, onCreate }: { creating: boolean; onCloseCreate: () => void; onCreate: () => void }) {
  const business = useCurrentBusiness();
  const rules = useRules();
  const [editing, setEditing] = useState<Rule | null>(null);
  const order = useApiMutation(
    (ids: string[]) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/allocations/rules/order', {
          params: { path: { business_id: business.id } },
          body: { ids },
        }),
      ),
    { invalidate: allocationChangeKeys(business.id) },
  );
  const archive = useApiMutation(
    ({ id, archived }: { id: string; archived: boolean }) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/rules/{rule_id}/archive', {
          params: { path: { business_id: business.id, rule_id: id } },
          body: { archived },
        }),
      ),
    { successMessage: (_, v) => (v.archived ? 'Rule archived' : 'Rule restored'), invalidate: allocationChangeKeys(business.id) },
  );

  let content;
  if (rules.isPending) content = <ListSkeleton rows={4} height={60} />;
  else if (rules.isError) content = <ErrorState error={rules.error} onRetry={() => void rules.refetch()} />;
  else {
    const active = rules.data.filter((r) => !r.archived);
    const archived = rules.data.filter((r) => r.archived);
    const move = (index: number, delta: number) => {
      const ids = active.map((r) => r.id);
      const [moved] = ids.splice(index, 1);
      ids.splice(index + delta, 0, moved!);
      order.mutate(ids);
    };
    const card = (rule: Rule, index: number) => (
      <RuleCard
        key={rule.id}
        rule={rule}
        index={index}
        count={active.length}
        currency={business.currency}
        onEdit={() => setEditing(rule)}
        onMove={(delta) => move(index, delta)}
        onArchive={(value) => archive.mutate({ id: rule.id, archived: value })}
        busy={order.isPending || archive.isPending}
      />
    );
    content =
      active.length === 0 && archived.length === 0 ? (
        <EmptyBlock
          icon={ListOrdered}
          title="No rules yet"
          description="A rule sets aside part of each payment you receive, for example 20% for income tax. Rules apply in order, each to what is left."
          action={<Button onClick={onCreate}>Add rule</Button>}
        />
      ) : (
        <Stack gap="xs">
          {active.length === 0 ? (
            <Text size="sm" c="dimmed">
              All your rules are archived, so nothing is proposed for new payments.
            </Text>
          ) : (
            active.map(card)
          )}
          {archived.length > 0 && (
            <>
              <Text size="sm" fw={600} mt="md">
                Archived
              </Text>
              {archived.map((r, i) => card(r, i))}
            </>
          )}
        </Stack>
      );
  }

  return (
    <Stack gap="lg">
      <SectionCard
        title="Rules"
        description="Applied in this order to each payment you receive, each to what earlier rules leave."
        actions={
          rules.data && rules.data.length > 0 ? (
            <Button size="xs" leftSection={<Plus size={14} />} onClick={onCreate}>
              Add rule
            </Button>
          ) : undefined
        }
      >
        {content}
      </SectionCard>
      {rules.data?.some((r) => !r.archived) && <TryRules currency={business.currency} today={business.today} />}
      <RuleFormModal opened={creating} onClose={onCloseCreate} />
      <RuleFormModal opened={editing !== null} onClose={() => setEditing(null)} rule={editing} />
    </Stack>
  );
}
