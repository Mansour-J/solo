import { Badge, Button, Group, Pagination, Paper, Stack, Table, Text, TextInput, UnstyledButton } from '@mantine/core';
import { useDebouncedValue, useDisclosure } from '@mantine/hooks';
import { HandCoins, Plus, Search } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { useSearchParams } from 'react-router';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { periodRange, PERIOD_LABELS, type Period } from '@/lib/periods';
import { useIncomeList } from './api';
import { IncomeFormModal } from './IncomeFormModal';
import { GST_TREATMENTS, INCOME_TYPES, optionLabel, PAYMENT_METHODS, type IncomeRow } from './incomeForm';

const PAGE_SIZE = 25;

/** Description, reference and customer, for the line under the payer. */
function details(row: IncomeRow): string {
  return [row.description, row.reference, row.customer_name ? `Customer: ${row.customer_name}` : ''].filter(Boolean).join(' · ');
}

function IncomeTable({ rows, showGst, onOpen }: { rows: IncomeRow[]; showGst: boolean; onOpen?: (row: IncomeRow) => void }) {
  return (
    <Table.ScrollContainer minWidth={720}>
      <Table highlightOnHover={!!onOpen} verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Date</Table.Th>
            <Table.Th>Payer</Table.Th>
            <Table.Th>Type</Table.Th>
            <Table.Th>Method</Table.Th>
            {showGst && <Table.Th ta="right">GST</Table.Th>}
            <Table.Th ta="right">Amount</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {rows.map((row) => (
            <Table.Tr
              key={row.id}
              style={onOpen ? { cursor: 'pointer' } : undefined}
              onClick={onOpen ? () => onOpen(row) : undefined}
              tabIndex={onOpen ? 0 : undefined}
              onKeyDown={
                onOpen
                  ? (event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        onOpen(row);
                      }
                    }
                  : undefined
              }
              aria-label={onOpen ? `Edit income from ${row.payer}` : undefined}
            >
              <Table.Td style={{ whiteSpace: 'nowrap' }}>
                <Text size="sm">{formatDate(row.income_date)}</Text>
              </Table.Td>
              <Table.Td>
                <Text size="sm" fw={500}>
                  {row.payer}
                </Text>
                {details(row) && (
                  <Text size="xs" c="dimmed" lineClamp={1}>
                    {details(row)}
                  </Text>
                )}
              </Table.Td>
              <Table.Td>
                <Badge variant="light" color={row.income_type === 'sales' ? 'brand' : 'gray'}>
                  {optionLabel(INCOME_TYPES, row.income_type)}
                </Badge>
              </Table.Td>
              <Table.Td>
                <Text size="sm">{optionLabel(PAYMENT_METHODS, row.payment_method)}</Text>
              </Table.Td>
              {showGst && (
                <Table.Td ta="right">
                  {row.gst_treatment === 'standard' ? (
                    <MoneyText value={row.gst_amount} currency={row.currency} size="sm" />
                  ) : (
                    <Text size="sm" c="dimmed">
                      {optionLabel(GST_TREATMENTS, row.gst_treatment)}
                    </Text>
                  )}
                </Table.Td>
              )}
              <Table.Td ta="right">
                <MoneyText value={row.gross_amount} currency={row.currency} size="sm" fw={600} />
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

function IncomeCards({ rows, onOpen }: { rows: IncomeRow[]; onOpen?: (row: IncomeRow) => void }) {
  return (
    <Stack gap="xs">
      {rows.map((row) => {
        const body = (
          <Group justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
            <Stack gap={2} style={{ minWidth: 0 }}>
              <Text fw={600} className="clamp-1">
                {row.payer}
              </Text>
              <Text size="xs" c="dimmed" className="clamp-1">
                {[formatDate(row.income_date), optionLabel(INCOME_TYPES, row.income_type), details(row)].filter(Boolean).join(' · ')}
              </Text>
            </Stack>
            <MoneyText value={row.gross_amount} currency={row.currency} fw={600} style={{ flexShrink: 0 }} />
          </Group>
        );
        return (
          <Paper key={row.id} withBorder radius="md" p="md" bg="var(--app-surface-raised)">
            {onOpen ? (
              <UnstyledButton w="100%" onClick={() => onOpen(row)} aria-label={`Edit income from ${row.payer}`}>
                {body}
              </UnstyledButton>
            ) : (
              body
            )}
          </Paper>
        );
      })}
    </Stack>
  );
}

export function IncomePage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  // `?new=1` opens the form (the dashboard’s quick action links here).
  const [searchParams, setSearchParams] = useSearchParams();
  const creating = searchParams.get('new') === '1';
  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search.trim(), 300);
  const [page, setPage] = useState(1);
  const [editing, setEditing] = useState<IncomeRow | null>(null);
  const [editOpened, editModal] = useDisclosure(false);
  const list = useIncomeList(business.id, {
    from: period.from,
    to: period.to,
    search: debouncedSearch || undefined,
    page,
    per_page: PAGE_SIZE,
  });

  const setCreating = (open: boolean) =>
    setSearchParams(
      (params) => {
        const next = new URLSearchParams(params);
        if (open) next.set('new', '1');
        else next.delete('new');
        return next;
      },
      { replace: true },
    );
  const openEdit = (row: IncomeRow) => {
    setEditing(row);
    editModal.open();
  };

  const recordButton = (
    <Button leftSection={<Plus size={16} />} onClick={() => setCreating(true)}>
      Record income
    </Button>
  );

  let content: ReactNode;
  if (list.isPending) {
    content = <ListSkeleton rows={8} />;
  } else if (list.isError) {
    content = <ErrorState error={list.error} onRetry={() => void list.refetch()} />;
  } else if (list.data.items.length === 0) {
    content = debouncedSearch ? (
      <EmptyBlock
        icon={Search}
        title="No matching income"
        description="Nothing in this period matches your search."
        action={
          <Button
            variant="default"
            onClick={() => {
              setSearch('');
              setPage(1);
            }}
          >
            Clear search
          </Button>
        }
      />
    ) : (
      <EmptyBlock
        icon={HandCoins}
        title="No income in this period"
        description="Record money you receive without an invoice, such as cash sales, interest or grants. Payments on invoices are recorded on the invoice."
        action={recordButton}
      />
    );
  } else {
    const { items, total } = list.data;
    content = (
      <Stack gap="md">
        {isMobile ? (
          <IncomeCards rows={items} onOpen={openEdit} />
        ) : (
          <IncomeTable rows={items} showGst={business.gst_registered} onOpen={openEdit} />
        )}
        <Group justify="space-between" gap="sm">
          <Text size="sm" c="dimmed">
            {total === 1 ? '1 record' : `${total} records`}
          </Text>
          {total > PAGE_SIZE && <Pagination total={Math.ceil(total / PAGE_SIZE)} value={page} onChange={setPage} size="sm" />}
        </Group>
      </Stack>
    );
  }

  return (
    <Page title="Income" description="Money you receive without an invoice, such as cash sales, interest or grants." actions={recordButton}>
      <SectionCard padding={isMobile ? 'sm' : 'lg'}>
        <Stack gap="md">
          <Group gap="sm" wrap="wrap" align="flex-start" justify="space-between">
            <Group gap="sm" wrap="wrap" style={{ flex: '1 1 auto' }}>
              <PeriodPicker
                value={period}
                onChange={(next) => {
                  setPeriod(next);
                  setPage(1);
                }}
                today={business.today}
                fyStart={business.current_financial_year.start}
              />
              <TextInput
                aria-label="Search income"
                placeholder="Search payer, description or reference"
                leftSection={<Search size={15} />}
                value={search}
                onChange={(event) => {
                  setSearch(event.currentTarget.value);
                  setPage(1);
                }}
                style={{ flex: '1 1 240px' }}
                maw={isMobile ? undefined : 360}
              />
            </Group>
            {list.data && (
              <Stack gap={0} align={isMobile ? 'flex-start' : 'flex-end'}>
                <Text size="xs" c="dimmed" fw={500} tt="uppercase" style={{ letterSpacing: '0.04em' }}>
                  Total received
                </Text>
                <MoneyText value={list.data.total_gross} currency={business.currency} fz="xl" fw={700} />
                <Text size="xs" c="dimmed">
                  {period.key === 'custom' ? `${formatDate(period.from)} – ${formatDate(period.to)}` : PERIOD_LABELS[period.key]}
                  {debouncedSearch ? ', matching your search' : ''}
                </Text>
              </Stack>
            )}
          </Group>
          {content}
        </Stack>
      </SectionCard>
      <IncomeFormModal opened={creating} onClose={() => setCreating(false)} />
      <IncomeFormModal opened={editOpened} onClose={editModal.close} income={editing} />
    </Page>
  );
}
