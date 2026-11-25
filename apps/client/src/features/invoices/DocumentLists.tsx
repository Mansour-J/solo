import { Button, Group, Pagination, Paper, SegmentedControl, Select, Stack, Table, Text, TextInput, UnstyledButton } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { ArrowDown, ArrowUp, ArrowUpDown, FileText, Plus, Search } from 'lucide-react';
import { useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { MoneyText, StatusBadge } from '@/components/Money';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { useInvoices, useQuotes } from './api';

const INVOICE_TABS = [
  { value: 'all', label: 'All', status: undefined },
  { value: 'draft', label: 'Drafts', status: 'draft' },
  { value: 'unpaid', label: 'Unpaid', status: 'issued,sent,partially_paid,overdue' },
  { value: 'overdue', label: 'Overdue', status: 'overdue' },
  { value: 'paid', label: 'Paid', status: 'paid' },
  { value: 'cancelled', label: 'Cancelled', status: 'cancelled' },
];

const QUOTE_TABS = [
  { value: 'all', label: 'All', status: undefined },
  { value: 'open', label: 'Open', status: 'draft,sent' },
  { value: 'accepted', label: 'Accepted', status: 'accepted' },
  { value: 'converted', label: 'Invoiced', status: 'converted' },
  { value: 'closed', label: 'Declined or expired', status: 'rejected,expired' },
];

/** Invoice list columns the API sorts by (`?sort=`, `-` for descending). */
type SortColumn = 'number' | 'customer' | 'issue_date' | 'due_date' | 'status' | 'total' | 'balance';

const DEFAULT_SORT = '-issue_date';

const SORT_LABELS: Record<SortColumn, string> = {
  number: 'Number',
  customer: 'Customer',
  issue_date: 'Issued',
  due_date: 'Due',
  status: 'Status',
  total: 'Total',
  balance: 'Balance',
};

/** Every column both ways, for the phone layout's menu. */
const SORT_OPTIONS = (Object.keys(SORT_LABELS) as SortColumn[]).flatMap((column) => {
  const dates = column === 'issue_date' || column === 'due_date';
  const money = column === 'total' || column === 'balance';
  const [up, down] = dates ? ['oldest first', 'newest first'] : money ? ['smallest first', 'largest first'] : ['A–Z', 'Z–A'];
  return [
    { value: column, label: `${SORT_LABELS[column]}, ${up}` },
    { value: `-${column}`, label: `${SORT_LABELS[column]}, ${down}` },
  ];
});

interface SortState {
  value: string;
  onChange: (value: string) => void;
}

/** A column header that sorts the list: first click ascending, the next reverses it. */
function SortHeader({ column, sort, align }: { column: SortColumn; sort: SortState; align?: 'right' }) {
  const direction = sort.value === column ? 'ascending' : sort.value === `-${column}` ? 'descending' : 'none';
  const Icon = direction === 'ascending' ? ArrowUp : direction === 'descending' ? ArrowDown : ArrowUpDown;
  return (
    <Table.Th ta={align} aria-sort={direction}>
      <UnstyledButton
        onClick={() => sort.onChange(direction === 'ascending' ? `-${column}` : column)}
        aria-label={`Sort by ${SORT_LABELS[column].toLowerCase()}`}
        style={{ display: 'inline-flex', alignItems: 'center', gap: 4, font: 'inherit', fontWeight: 'inherit' }}
      >
        {SORT_LABELS[column]}
        <Icon size={13} style={{ opacity: direction === 'none' ? 0.35 : 1 }} aria-hidden />
      </UnstyledButton>
    </Table.Th>
  );
}

interface Row {
  id: string;
  number: string;
  customer: string;
  date: string;
  end: string;
  status: string;
  total: number;
  balance?: number;
}

function ListView({ rows, kind, currency, sort }: { rows: Row[]; kind: 'invoice' | 'quote'; currency: string; sort?: SortState }) {
  const isMobile = useIsMobile();
  const navigate = useNavigate();
  const href = (id: string) => `/${kind === 'invoice' ? 'invoices' : 'quotes'}/${id}`;
  if (isMobile) {
    return (
      <Stack gap="xs">
        {rows.map((r) => (
          <UnstyledButton key={r.id} component={Link} to={href(r.id)}>
            <Paper withBorder p="sm" radius="md">
              <Group justify="space-between" wrap="nowrap">
                <div style={{ minWidth: 0 }}>
                  <Text fw={600} className="clamp-1">
                    {r.customer}
                  </Text>
                  <Text size="xs" c="dimmed">
                    {r.number} · {formatDate(r.date)}
                  </Text>
                </div>
                <Stack gap={4} align="flex-end">
                  <MoneyText value={r.balance ?? r.total} currency={currency} fw={600} />
                  <StatusBadge status={r.status} />
                </Stack>
              </Group>
            </Paper>
          </UnstyledButton>
        ))}
      </Stack>
    );
  }
  return (
    <Paper withBorder radius="lg" bg="var(--app-surface-raised)">
      <Table.ScrollContainer minWidth={720}>
        <Table highlightOnHover verticalSpacing="sm">
          <Table.Thead>
            {sort ? (
              <Table.Tr>
                <SortHeader column="number" sort={sort} />
                <SortHeader column="customer" sort={sort} />
                <SortHeader column="issue_date" sort={sort} />
                <SortHeader column="due_date" sort={sort} />
                <SortHeader column="status" sort={sort} />
                <SortHeader column="total" sort={sort} align="right" />
                <SortHeader column="balance" sort={sort} align="right" />
              </Table.Tr>
            ) : (
              <Table.Tr>
                <Table.Th>Number</Table.Th>
                <Table.Th>Customer</Table.Th>
                <Table.Th>{kind === 'invoice' ? 'Issued' : 'Date'}</Table.Th>
                <Table.Th>{kind === 'invoice' ? 'Due' : 'Valid until'}</Table.Th>
                <Table.Th>Status</Table.Th>
                <Table.Th ta="right">Total</Table.Th>
                {kind === 'invoice' && <Table.Th ta="right">Balance</Table.Th>}
              </Table.Tr>
            )}
          </Table.Thead>
          <Table.Tbody>
            {rows.map((r) => (
              <Table.Tr key={r.id} style={{ cursor: 'pointer' }} onClick={() => navigate(href(r.id))}>
                <Table.Td>
                  <Text component={Link} to={href(r.id)} fw={500} size="sm" onClick={(e) => e.stopPropagation()}>
                    {r.number}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Text size="sm" className="clamp-1">
                    {r.customer}
                  </Text>
                </Table.Td>
                <Table.Td>{formatDate(r.date)}</Table.Td>
                <Table.Td>{formatDate(r.end)}</Table.Td>
                <Table.Td>
                  <StatusBadge status={r.status} />
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={r.total} currency={currency} size="sm" />
                </Table.Td>
                {kind === 'invoice' && (
                  <Table.Td ta="right">
                    <MoneyText
                      value={r.balance ?? 0}
                      currency={currency}
                      size="sm"
                      fw={(r.balance ?? 0) > 0 ? 600 : undefined}
                      c={(r.balance ?? 0) > 0 ? undefined : 'dimmed'}
                    />
                  </Table.Td>
                )}
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
    </Paper>
  );
}

function useListState(tabs: { value: string }[]) {
  const [params, setParams] = useSearchParams();
  const tab = tabs.some((t) => t.value === params.get('tab')) ? (params.get('tab') as string) : 'all';
  const [search, setSearch] = useState('');
  const [debounced] = useDebouncedValue(search.trim(), 250);
  const [page, setPage] = useState(1);
  return {
    tab,
    setTab: (value: string) => {
      setPage(1);
      setParams(value === 'all' ? {} : { tab: value }, { replace: true });
    },
    search,
    setSearch: (value: string) => {
      setPage(1);
      setSearch(value);
    },
    debounced,
    page,
    setPage,
  };
}

const PER_PAGE = 25;

export function InvoicesPage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const state = useListState(INVOICE_TABS);
  const [sortValue, setSortValue] = useState(DEFAULT_SORT);
  const sort: SortState = {
    value: sortValue,
    onChange: (value) => {
      state.setPage(1);
      setSortValue(value);
    },
  };
  const status = INVOICE_TABS.find((t) => t.value === state.tab)?.status;
  const invoices = useInvoices(business.id, {
    status,
    search: state.debounced || undefined,
    sort: sortValue,
    page: state.page,
    per_page: PER_PAGE,
  });
  const rows: Row[] = (invoices.data?.items ?? []).map((i) => ({
    id: i.id,
    number: i.number ?? 'Draft',
    customer: i.customer_name,
    date: i.issue_date,
    end: i.due_date,
    status: i.display_status,
    total: i.total,
    balance: i.status === 'issued' ? i.balance : 0,
  }));
  const filtered = state.tab !== 'all' || !!state.debounced;
  return (
    <Page
      title="Invoices"
      actions={
        <Group gap="xs">
          <Button variant="default" component={Link} to="/quotes">
            Quotes
          </Button>
          <Button variant="default" component={Link} to="/invoices/recurring">
            Recurring
          </Button>
          <Button component={Link} to="/invoices/new" leftSection={<Plus size={16} />}>
            New invoice
          </Button>
        </Group>
      }
    >
      <Group justify="space-between" gap="sm">
        <SegmentedControl
          value={state.tab}
          onChange={state.setTab}
          data={INVOICE_TABS.map(({ value, label }) => ({ value, label }))}
          size="sm"
        />
        <TextInput
          aria-label="Search invoices"
          placeholder="Search number, reference or customer"
          leftSection={<Search size={15} />}
          value={state.search}
          onChange={(e) => state.setSearch(e.currentTarget.value)}
          w={{ base: '100%', sm: 300 }}
        />
        {isMobile && (
          <Select
            aria-label="Sort invoices"
            w="100%"
            allowDeselect={false}
            value={sortValue}
            data={SORT_OPTIONS}
            onChange={(value) => value && sort.onChange(value)}
          />
        )}
      </Group>
      {invoices.isPending ? (
        <ListSkeleton rows={8} />
      ) : invoices.isError ? (
        <ErrorState error={invoices.error} onRetry={() => void invoices.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyBlock
          icon={FileText}
          title={filtered ? 'No invoices match' : 'No invoices yet'}
          description={
            filtered
              ? 'Try another filter or search.'
              : 'Create your first invoice. You can save it as a draft and issue it when you’re ready.'
          }
          action={
            !filtered && (
              <Button component={Link} to="/invoices/new" leftSection={<Plus size={16} />}>
                New invoice
              </Button>
            )
          }
        />
      ) : (
        <Stack>
          <ListView rows={rows} kind="invoice" currency={business.currency} sort={sort} />
          {(invoices.data?.total ?? 0) > PER_PAGE && (
            <Group justify="flex-end">
              <Pagination total={Math.ceil((invoices.data?.total ?? 0) / PER_PAGE)} value={state.page} onChange={state.setPage} size="sm" />
            </Group>
          )}
        </Stack>
      )}
    </Page>
  );
}

export function QuotesPage() {
  const business = useCurrentBusiness();
  const state = useListState(QUOTE_TABS);
  const status = QUOTE_TABS.find((t) => t.value === state.tab)?.status;
  const quotes = useQuotes(business.id, { status, search: state.debounced || undefined, page: state.page, per_page: PER_PAGE });
  const rows: Row[] = (quotes.data?.items ?? []).map((q) => ({
    id: q.id,
    number: q.number,
    customer: q.customer_name,
    date: q.issue_date,
    end: q.expiry_date,
    status: q.display_status,
    total: q.total,
  }));
  const filtered = state.tab !== 'all' || !!state.debounced;
  return (
    <Page
      title="Quotes"
      actions={
        <Group gap="xs">
          <Button variant="default" component={Link} to="/invoices">
            Invoices
          </Button>
          <Button component={Link} to="/quotes/new" leftSection={<Plus size={16} />}>
            New quote
          </Button>
        </Group>
      }
    >
      <Group justify="space-between" gap="sm">
        <SegmentedControl
          value={state.tab}
          onChange={state.setTab}
          data={QUOTE_TABS.map(({ value, label }) => ({ value, label }))}
          size="sm"
        />
        <TextInput
          aria-label="Search quotes"
          placeholder="Search number, reference or customer"
          leftSection={<Search size={15} />}
          value={state.search}
          onChange={(e) => state.setSearch(e.currentTarget.value)}
          w={{ base: '100%', sm: 300 }}
        />
      </Group>
      {quotes.isPending ? (
        <ListSkeleton rows={8} />
      ) : quotes.isError ? (
        <ErrorState error={quotes.error} onRetry={() => void quotes.refetch()} />
      ) : rows.length === 0 ? (
        <EmptyBlock
          icon={FileText}
          title={filtered ? 'No quotes match' : 'No quotes yet'}
          description={
            filtered ? 'Try another filter or search.' : 'Send customers a quote, then turn it into an invoice when they accept.'
          }
          action={
            !filtered && (
              <Button component={Link} to="/quotes/new" leftSection={<Plus size={16} />}>
                New quote
              </Button>
            )
          }
        />
      ) : (
        <Stack>
          <ListView rows={rows} kind="quote" currency={business.currency} />
          {(quotes.data?.total ?? 0) > PER_PAGE && (
            <Group justify="flex-end">
              <Pagination total={Math.ceil((quotes.data?.total ?? 0) / PER_PAGE)} value={state.page} onChange={state.setPage} size="sm" />
            </Group>
          )}
        </Stack>
      )}
    </Page>
  );
}
