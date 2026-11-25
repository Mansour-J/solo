import { ActionIcon, Badge, Box, Button, Group, Paper, Stack, Text, Tooltip } from '@mantine/core';
import { ArrowDown, ArrowUp, Archive, Pencil, Plus, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatMoney } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { allocationChangeKeys, useDestinations, type Destination } from './api';
import { DestinationFormModal } from './DestinationFormModal';
import { KIND_OPTIONS } from './format';

function kindLabel(kind: string): string {
  return KIND_OPTIONS.find((k) => k.value === kind)?.label ?? kind;
}

function details(d: Destination, currency: string): string {
  const parts = [kindLabel(d.kind)];
  if (d.target_amount) parts.push(`Target ${formatMoney(d.target_amount, currency)}`);
  if (d.minimum_balance) parts.push(`Minimum ${formatMoney(d.minimum_balance, currency)}`);
  parts.push(d.rule_count === 1 ? '1 rule' : `${d.rule_count} rules`);
  return parts.join(' · ');
}

/** Destinations in display order, with balances, targets and archiving. */
export function DestinationsPanel() {
  const business = useCurrentBusiness();
  const destinations = useDestinations();
  const [editing, setEditing] = useState<Destination | null>(null);
  const [adding, setAdding] = useState(false);
  const order = useApiMutation(
    (ids: string[]) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/allocations/destinations/order', {
          params: { path: { business_id: business.id } },
          body: { ids },
        }),
      ),
    { invalidate: allocationChangeKeys(business.id) },
  );
  const archive = useApiMutation(
    ({ id, archived }: { id: string; archived: boolean }) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/allocations/destinations/{destination_id}/archive', {
          params: { path: { business_id: business.id, destination_id: id } },
          body: { archived },
        }),
      ),
    { successMessage: (d, v) => (v.archived ? `${d.name} archived` : `${d.name} restored`), invalidate: allocationChangeKeys(business.id) },
  );

  let content;
  if (destinations.isPending) content = <ListSkeleton rows={6} height={56} />;
  else if (destinations.isError) content = <ErrorState error={destinations.error} onRetry={() => void destinations.refetch()} />;
  else {
    const active = destinations.data.filter((d) => !d.archived);
    const archived = destinations.data.filter((d) => d.archived);
    const busy = order.isPending || archive.isPending;
    const move = (index: number, delta: number) => {
      const ids = active.map((d) => d.id);
      const [moved] = ids.splice(index, 1);
      ids.splice(index + delta, 0, moved!);
      order.mutate(ids);
    };
    const row = (d: Destination, index: number) => (
      <Paper key={d.id} withBorder radius="md" p="sm">
        <Group justify="space-between" wrap="wrap" gap="xs">
          <Box style={{ minWidth: 0, flex: '1 1 200px' }}>
            <Group gap={6} wrap="nowrap">
              <Text size="sm" fw={500} className="clamp-1">
                {d.name}
              </Text>
              {d.minimum_balance != null && d.balance < d.minimum_balance && (
                <Badge size="xs" variant="light" color="orange">
                  Below minimum
                </Badge>
              )}
            </Group>
            <Text size="xs" c="dimmed" className="clamp-1">
              {details(d, business.currency)}
            </Text>
          </Box>
          <Group gap="sm" wrap="nowrap" justify="flex-end" style={{ flexShrink: 0, marginLeft: 'auto' }}>
            <MoneyText value={d.balance} currency={business.currency} size="sm" fw={500} />
            {
              <Group gap={2} wrap="nowrap">
                {!d.archived && (
                  <>
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      aria-label={`Move ${d.name} up`}
                      disabled={index === 0 || busy}
                      onClick={() => move(index, -1)}
                    >
                      <ArrowUp size={16} />
                    </ActionIcon>
                    <ActionIcon
                      variant="subtle"
                      color="gray"
                      aria-label={`Move ${d.name} down`}
                      disabled={index === active.length - 1 || busy}
                      onClick={() => move(index, 1)}
                    >
                      <ArrowDown size={16} />
                    </ActionIcon>
                    <ActionIcon variant="subtle" color="gray" aria-label={`Edit ${d.name}`} onClick={() => setEditing(d)}>
                      <Pencil size={16} />
                    </ActionIcon>
                  </>
                )}
                <Tooltip label={d.archived ? 'Restore' : 'Archive'}>
                  <ActionIcon
                    variant="subtle"
                    color="gray"
                    aria-label={d.archived ? `Restore ${d.name}` : `Archive ${d.name}`}
                    disabled={busy}
                    onClick={() => archive.mutate({ id: d.id, archived: !d.archived })}
                  >
                    {d.archived ? <RotateCcw size={16} /> : <Archive size={16} />}
                  </ActionIcon>
                </Tooltip>
              </Group>
            }
          </Group>
        </Group>
      </Paper>
    );
    content = (
      <Stack gap="xs">
        {active.map(row)}
        {archived.length > 0 && (
          <>
            <Text size="sm" fw={600} mt="md">
              Archived
            </Text>
            {archived.map(row)}
          </>
        )}
      </Stack>
    );
  }

  return (
    <SectionCard
      title="Destinations"
      description="Where you plan for money to go. Archive the ones you don’t use; their history stays."
      actions={
        <Button size="xs" leftSection={<Plus size={14} />} onClick={() => setAdding(true)}>
          Add destination
        </Button>
      }
    >
      {content}
      <DestinationFormModal opened={adding} onClose={() => setAdding(false)} />
      <DestinationFormModal opened={editing !== null} onClose={() => setEditing(null)} destination={editing} />
    </SectionCard>
  );
}
