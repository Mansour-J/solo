import { Anchor, Badge, Button, Group, Pagination, Paper, SegmentedControl, Select, Stack, Table, Text, TextInput } from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import { Plus, Search, Users } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { useCustomers, type CustomerListParams } from './api';
import { CustomerFormModal } from './CustomerFormModal';
import { SORT_OPTIONS, STATUS_FILTERS, type Customer } from './customerForm';

const PAGE_SIZE = 25;

type Status = NonNullable<CustomerListParams['status']>;
type Sort = NonNullable<CustomerListParams['sort']>;

function secondaryName(customer: Customer): string {
  return customer.business_name && customer.business_name !== customer.name ? customer.business_name : '';
}

function InactiveBadge() {
  return (
    <Badge component="span" color="gray" variant="light" size="sm">
      Inactive
    </Badge>
  );
}

function CustomerTable({ customers, currency }: { customers: Customer[]; currency: string }) {
  const navigate = useNavigate();
  return (
    <Table.ScrollContainer minWidth={640}>
      <Table highlightOnHover verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Name</Table.Th>
            <Table.Th>Email</Table.Th>
            <Table.Th ta="right">Outstanding</Table.Th>
            <Table.Th ta="right">Overdue</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {customers.map((customer) => (
            <Table.Tr
              key={customer.id}
              style={{ cursor: 'pointer' }}
              onClick={(event) => {
                // The name is a link already; don't navigate twice.
                if ((event.target as HTMLElement).closest('a')) return;
                navigate(`/customers/${customer.id}`);
              }}
            >
              <Table.Td>
                <Group gap="xs" wrap="nowrap">
                  <Anchor component={Link} to={`/customers/${customer.id}`} fw={500} c="inherit">
                    {customer.name}
                  </Anchor>
                  {!customer.is_active && <InactiveBadge />}
                </Group>
                {secondaryName(customer) && (
                  <Text size="xs" c="dimmed">
                    {secondaryName(customer)}
                  </Text>
                )}
              </Table.Td>
              <Table.Td>
                <Text size="sm" c={customer.email ? undefined : 'dimmed'}>
                  {customer.email ?? '—'}
                </Text>
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={customer.outstanding} currency={currency} size="sm" c={customer.outstanding > 0 ? undefined : 'dimmed'} />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText
                  value={customer.overdue}
                  currency={currency}
                  size="sm"
                  c={customer.overdue > 0 ? 'red' : 'dimmed'}
                  fw={customer.overdue > 0 ? 600 : undefined}
                />
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

function CustomerCards({ customers, currency }: { customers: Customer[]; currency: string }) {
  return (
    <Stack gap="xs">
      {customers.map((customer) => (
        <Paper
          key={customer.id}
          component={Link}
          to={`/customers/${customer.id}`}
          withBorder
          radius="md"
          p="md"
          bg="var(--app-surface-raised)"
          style={{ color: 'inherit', textDecoration: 'none' }}
        >
          <Group justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
            <Stack gap={2} style={{ minWidth: 0 }}>
              <Group gap="xs" wrap="nowrap">
                <Text fw={600} className="clamp-1">
                  {customer.name}
                </Text>
                {!customer.is_active && <InactiveBadge />}
              </Group>
              {(secondaryName(customer) || customer.email) && (
                <Text size="xs" c="dimmed" className="clamp-1">
                  {secondaryName(customer) || customer.email}
                </Text>
              )}
            </Stack>
            <Stack gap={0} align="flex-end" style={{ flexShrink: 0 }}>
              <MoneyText value={customer.outstanding} currency={currency} fw={600} c={customer.outstanding > 0 ? undefined : 'dimmed'} />
              {customer.overdue > 0 && (
                <Text size="xs" c="red" fw={500}>
                  <MoneyText value={customer.overdue} currency={currency} inherit /> overdue
                </Text>
              )}
            </Stack>
          </Group>
        </Paper>
      ))}
    </Stack>
  );
}

export function CustomersPage() {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const isMobile = useIsMobile();
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search.trim(), 300);
  const [status, setStatus] = useState<Status>('active');
  const [sort, setSort] = useState<Sort>('name');
  const [page, setPage] = useState(1);
  // `?new=1` opens the add form (the dashboard’s quick action links here).
  const [searchParams, setSearchParams] = useSearchParams();
  const creating = searchParams.get('new') === '1';
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
  const list = useCustomers(business.id, { search: debouncedSearch || undefined, status, sort, page, per_page: PAGE_SIZE });
  const filtered = debouncedSearch !== '' || status !== 'active';

  const addButton = (
    <Button leftSection={<Plus size={16} />} onClick={() => setCreating(true)}>
      Add customer
    </Button>
  );

  let content: ReactNode;
  if (list.isPending) {
    content = <ListSkeleton rows={8} />;
  } else if (list.isError) {
    content = <ErrorState error={list.error} onRetry={() => void list.refetch()} />;
  } else if (list.data.items.length === 0) {
    content = filtered ? (
      <EmptyBlock
        icon={Search}
        title="No matching customers"
        description="Try a different search or status."
        action={
          <Button
            variant="default"
            onClick={() => {
              setSearch('');
              setStatus('active');
              setPage(1);
            }}
          >
            Clear filters
          </Button>
        }
      />
    ) : (
      <EmptyBlock
        icon={Users}
        title="No customers yet"
        description="Add the people and organisations you work for. Their details fill in automatically on invoices and quotes."
        action={addButton}
      />
    );
  } else {
    const { items, total } = list.data;
    content = (
      <Stack gap="md">
        {isMobile ? (
          <CustomerCards customers={items} currency={business.currency} />
        ) : (
          <CustomerTable customers={items} currency={business.currency} />
        )}
        <Group justify="space-between" gap="sm">
          <Text size="sm" c="dimmed">
            {total === 1 ? '1 customer' : `${total} customers`}
          </Text>
          {total > PAGE_SIZE && <Pagination total={Math.ceil(total / PAGE_SIZE)} value={page} onChange={setPage} size="sm" />}
        </Group>
      </Stack>
    );
  }

  return (
    <Page title="Customers" description="The people and organisations you invoice." actions={addButton}>
      <SectionCard padding={isMobile ? 'sm' : 'lg'}>
        <Stack gap="md">
          <Group gap="sm" wrap="wrap" align="flex-end">
            <TextInput
              aria-label="Search customers"
              placeholder="Search name, business, contact or email"
              leftSection={<Search size={15} />}
              value={search}
              onChange={(event) => {
                setSearch(event.currentTarget.value);
                setPage(1);
              }}
              style={{ flex: '1 1 260px' }}
            />
            <SegmentedControl
              aria-label="Status"
              value={status}
              data={STATUS_FILTERS.map((s) => ({ ...s }))}
              onChange={(value) => {
                setStatus(value as Status);
                setPage(1);
              }}
            />
            <Select
              aria-label="Sort by"
              w={isMobile ? '100%' : 190}
              allowDeselect={false}
              value={sort}
              data={SORT_OPTIONS.map((s) => ({ value: s.value, label: `Sort: ${s.label}` }))}
              onChange={(value) => {
                if (!value) return;
                setSort(value as Sort);
                setPage(1);
              }}
            />
          </Group>
          {content}
        </Stack>
      </SectionCard>
      <CustomerFormModal opened={creating} onClose={() => setCreating(false)} onSaved={(saved) => navigate(`/customers/${saved.id}`)} />
    </Page>
  );
}
