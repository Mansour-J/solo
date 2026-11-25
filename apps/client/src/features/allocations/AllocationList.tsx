import { Badge, Box, Group, Pagination, Paper, Stack, Table, Text, UnstyledButton } from '@mantine/core';
import { useState, type ReactNode } from 'react';
import { MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { useAllocations, type Allocation, type AllocationListParams } from './api';
import { sourceText, statusOf } from './format';

const PER_PAGE = 20;

function label(a: Allocation): string {
  return `${sourceText(a.source_type)} of ${formatDate(a.received_date)}: ${a.source_label}`;
}

function StatusCell({ a }: { a: Allocation }) {
  const status = statusOf(a.status);
  return (
    <Badge variant="light" color={status.color}>
      {status.label}
    </Badge>
  );
}

function Rows({ items, onOpen }: { items: Allocation[]; onOpen: (a: Allocation) => void }) {
  return (
    <Paper withBorder radius="lg" style={{ overflow: 'hidden' }}>
      <Table.ScrollContainer minWidth={720}>
        <Table highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th w={110}>Received</Table.Th>
              <Table.Th>From</Table.Th>
              <Table.Th ta="right">Amount</Table.Th>
              <Table.Th ta="right">Allocated</Table.Th>
              <Table.Th w={120}>Status</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((a) => (
              <Table.Tr key={a.id} onClick={() => onOpen(a)} style={{ cursor: 'pointer' }}>
                <Table.Td style={{ whiteSpace: 'nowrap' }}>{formatDate(a.received_date)}</Table.Td>
                <Table.Td maw={320}>
                  {/* Focusable for keyboard users; its click bubbles to the row. */}
                  <UnstyledButton display="block" maw="100%" aria-label={label(a)}>
                    <Text size="sm" fw={500} truncate="end">
                      {a.source_label}
                    </Text>
                    <Text size="xs" c="dimmed" truncate="end">
                      {sourceText(a.source_type)}
                      {a.customer_name ? ` · ${a.customer_name}` : ''}
                    </Text>
                  </UnstyledButton>
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={a.received_amount} currency={a.currency} size="sm" />
                </Table.Td>
                <Table.Td ta="right">
                  {a.status === 'confirmed' || a.status === 'reversed' ? (
                    <MoneyText value={a.allocated} currency={a.currency} size="sm" />
                  ) : (
                    <Text size="sm" c="dimmed">
                      —
                    </Text>
                  )}
                </Table.Td>
                <Table.Td>
                  <StatusCell a={a} />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
    </Paper>
  );
}

function Cards({ items, onOpen }: { items: Allocation[]; onOpen: (a: Allocation) => void }) {
  return (
    <Stack gap="xs">
      {items.map((a) => (
        <UnstyledButton key={a.id} onClick={() => onOpen(a)} aria-label={label(a)}>
          <Paper withBorder radius="md" p="sm">
            <Group justify="space-between" wrap="nowrap" gap="sm" align="flex-start">
              <Box style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} truncate="end">
                  {a.source_label}
                </Text>
                <Text size="xs" c="dimmed" truncate="end">
                  {formatDate(a.received_date)} · {sourceText(a.source_type)}
                </Text>
              </Box>
              <Stack gap={4} align="flex-end" style={{ flexShrink: 0 }}>
                <MoneyText value={a.received_amount} currency={a.currency} size="sm" fw={600} />
                <StatusCell a={a} />
              </Stack>
            </Group>
          </Paper>
        </UnstyledButton>
      ))}
    </Stack>
  );
}

/** A paged list of allocations; clicking one opens it. `empty` shows when there are none. */
export function AllocationList({
  params,
  onOpen,
  empty,
  compact = false,
}: {
  params: Omit<AllocationListParams, 'page' | 'per_page'>;
  onOpen: (a: Allocation) => void;
  empty: ReactNode;
  /** Cards even on wide screens (a narrow column). */
  compact?: boolean;
}) {
  const isMobile = useIsMobile();
  const [page, setPage] = useState(1);
  const list = useAllocations({ ...params, page, per_page: PER_PAGE });
  if (list.isPending) return <ListSkeleton rows={4} height={isMobile ? 64 : 52} />;
  if (list.isError) return <ErrorState error={list.error} onRetry={() => void list.refetch()} />;
  if (list.data.items.length === 0) return <>{empty}</>;
  return (
    <Stack gap="sm" style={{ opacity: list.isPlaceholderData ? 0.6 : 1, transition: 'opacity 120ms' }}>
      {isMobile || compact ? <Cards items={list.data.items} onOpen={onOpen} /> : <Rows items={list.data.items} onOpen={onOpen} />}
      {list.data.total > PER_PAGE && (
        <Group justify="flex-end">
          <Pagination total={Math.ceil(list.data.total / PER_PAGE)} value={page} onChange={setPage} size="sm" />
        </Group>
      )}
    </Stack>
  );
}
