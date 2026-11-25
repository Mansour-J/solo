import {
  ActionIcon,
  Alert,
  Anchor,
  Badge,
  Button,
  Group,
  Menu,
  Pagination,
  Paper,
  SimpleGrid,
  Stack,
  Table,
  Tabs,
  Text,
} from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { useEmailEnabled } from '@/features/server/api';
import { CustomerRemindersSwitch } from '@/features/invoices/Reminders';
import { EmailStatementModal, StatementEmailsCard } from './StatementEmail';
import { useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, EllipsisVertical, FilePlus, FileText, Mail, Pencil, Power, RotateCcw, ScrollText, Trash2, UserX } from 'lucide-react';
import { useState, type MouseEvent, type ReactNode } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { confirmAction } from '@/components/confirmAction';
import { MoneyText, StatusBadge } from '@/components/Money';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { businessKeys, useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { formatDate, formatNumber } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { isApiError } from '@/services/api/errors';
import { useApiMutation } from '@/services/api/query';
import { customerKeys, HISTORY_PAGE_SIZE, useCustomer, useCustomerInvoices, useCustomerQuotes } from './api';
import { CustomerFormModal } from './CustomerFormModal';
import { addressLines, withActive, type Customer, type CustomerDetail } from './customerForm';

const backLink = (
  <Anchor component={Link} to="/customers" size="sm" c="dimmed">
    <Group gap={4} component="span">
      <ArrowLeft size={14} />
      Customers
    </Group>
  </Anchor>
);

// ─── Summary ────────────────────────────────────────────────────────────────

function Stat({ label, children, hint }: { label: string; children: ReactNode; hint?: ReactNode }) {
  return (
    <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
      <Text size="xs" c="dimmed" fw={500} tt="uppercase" style={{ letterSpacing: '0.04em' }}>
        {label}
      </Text>
      <Text fz="xl" fw={700} mt={4} className="tabular">
        {children}
      </Text>
      {hint && (
        <Text size="xs" c="dimmed" mt={2}>
          {hint}
        </Text>
      )}
    </Paper>
  );
}

function Summary({ customer, currency }: { customer: CustomerDetail; currency: string }) {
  const { stats } = customer;
  const averageDays = stats.average_days_to_pay;
  return (
    <SimpleGrid cols={{ base: 2, sm: 3, lg: averageDays == null ? 4 : 5 }} spacing="md">
      <Stat label="Outstanding">
        <MoneyText value={customer.outstanding} currency={currency} inherit />
      </Stat>
      <Stat label="Overdue">
        <MoneyText value={customer.overdue} currency={currency} inherit c={customer.overdue > 0 ? 'red' : undefined} />
      </Stat>
      <Stat
        label="Total invoiced"
        hint={
          stats.invoice_count > 0
            ? `${stats.invoice_count === 1 ? '1 invoice' : `${stats.invoice_count} invoices`}${stats.last_invoice_date ? `, last ${formatDate(stats.last_invoice_date)}` : ''}`
            : 'No invoices yet'
        }
      >
        <MoneyText value={stats.total_invoiced} currency={currency} inherit />
      </Stat>
      <Stat label="Total paid">
        <MoneyText value={stats.total_paid} currency={currency} inherit />
      </Stat>
      {averageDays != null && (
        <Stat label="Average time to pay" hint="From issue to final payment">
          {Math.round(averageDays) === 1 ? '1 day' : `${formatNumber(Math.round(averageDays))} days`}
        </Stat>
      )}
    </SimpleGrid>
  );
}

// ─── Contact details ────────────────────────────────────────────────────────

function Detail({ label, children }: { label: string; children: ReactNode }) {
  return (
    <Stack gap={2}>
      <Text size="xs" c="dimmed" fw={500}>
        {label}
      </Text>
      <Text size="sm" component="div" style={{ overflowWrap: 'anywhere' }}>
        {children}
      </Text>
    </Stack>
  );
}

function Lines({ lines }: { lines: string[] }) {
  return lines.map((line, i) => <div key={i}>{line}</div>);
}

function ContactDetails({ customer }: { customer: Customer }) {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  const billing = addressLines(customer.billing_address);
  const located = addressLines(customer.business_address);
  const sameAddress = billing.join('\n') === located.join('\n');
  const defaultTerms = settings.data?.default_payment_terms_days;
  const details: { label: string; value: ReactNode }[] = [];
  if (customer.business_name && customer.business_name !== customer.name)
    details.push({ label: 'Business name', value: customer.business_name });
  if (customer.contact_person) details.push({ label: 'Contact person', value: customer.contact_person });
  if (customer.email)
    details.push({
      label: 'Email',
      value: (
        <Anchor href={`mailto:${customer.email}`} size="sm">
          {customer.email}
        </Anchor>
      ),
    });
  if (customer.phone)
    details.push({
      label: 'Phone',
      value: (
        <Anchor href={`tel:${customer.phone.replace(/[^\d+]/g, '')}`} size="sm">
          {customer.phone}
        </Anchor>
      ),
    });
  if (billing.length) details.push({ label: sameAddress ? 'Address' : 'Billing address', value: <Lines lines={billing} /> });
  if (located.length && !sameAddress) details.push({ label: 'Business address', value: <Lines lines={located} /> });
  if (customer.nzbn) details.push({ label: 'NZBN', value: customer.nzbn });
  if (customer.tax_number) details.push({ label: 'Tax number', value: customer.tax_number });
  details.push({
    label: 'Payment terms',
    value:
      customer.payment_terms_days != null
        ? `${customer.payment_terms_days} days`
        : defaultTerms !== undefined
          ? `Your default (${defaultTerms} days)`
          : 'Your default',
  });

  return (
    <SectionCard title="Contact details">
      <SimpleGrid cols={{ base: 1, xs: 2, md: 4 }} spacing="lg" verticalSpacing="md">
        {details.map((d) => (
          <Detail key={d.label} label={d.label}>
            {d.value}
          </Detail>
        ))}
      </SimpleGrid>
    </SectionCard>
  );
}

// ─── History ────────────────────────────────────────────────────────────────

function HistoryPagination({ total, page, onChange }: { total: number; page: number; onChange: (page: number) => void }) {
  if (total <= HISTORY_PAGE_SIZE) return null;
  return (
    <Group justify="flex-end" mt="md">
      <Pagination total={Math.ceil(total / HISTORY_PAGE_SIZE)} value={page} onChange={onChange} size="sm" />
    </Group>
  );
}

/** Rows open the document; the number cell is a real link for keyboard and screen-reader users. */
function rowProps(to: string, navigate: (to: string) => void) {
  return {
    style: { cursor: 'pointer' },
    onClick: (event: MouseEvent) => {
      if ((event.target as HTMLElement).closest('a')) return;
      navigate(to);
    },
  };
}

function InvoiceHistory({ customer, canCreate }: { customer: Customer; canCreate: boolean }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const invoices = useCustomerInvoices(business.id, customer.id, page);
  if (invoices.isPending) return <ListSkeleton rows={3} height={40} />;
  if (invoices.isError) return <ErrorState error={invoices.error} onRetry={() => void invoices.refetch()} />;
  const { items, total } = invoices.data;
  if (items.length === 0) {
    return (
      <EmptyBlock
        compact
        icon={FileText}
        title="No invoices yet"
        action={
          canCreate && (
            <Button component={Link} to={`/invoices/new?customer=${customer.id}`} variant="light" leftSection={<FilePlus size={16} />}>
              New invoice
            </Button>
          )
        }
      />
    );
  }
  return (
    <>
      <Table.ScrollContainer minWidth={640}>
        <Table highlightOnHover verticalSpacing="sm">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Number</Table.Th>
              <Table.Th>Issued</Table.Th>
              <Table.Th>Due</Table.Th>
              <Table.Th>Status</Table.Th>
              <Table.Th ta="right">Total</Table.Th>
              <Table.Th ta="right">Balance</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((invoice) => (
              <Table.Tr key={invoice.id} {...rowProps(`/invoices/${invoice.id}`, navigate)}>
                <Table.Td>
                  <Anchor component={Link} to={`/invoices/${invoice.id}`} fw={500} size="sm" c={invoice.number ? 'inherit' : 'dimmed'}>
                    {invoice.number ?? 'Draft'}
                  </Anchor>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{formatDate(invoice.issue_date)}</Text>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{formatDate(invoice.due_date)}</Text>
                </Table.Td>
                <Table.Td>
                  <StatusBadge status={invoice.display_status} />
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={invoice.total} currency={invoice.currency} size="sm" />
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText
                    value={invoice.balance}
                    currency={invoice.currency}
                    size="sm"
                    c={invoice.display_status === 'overdue' ? 'red' : invoice.balance === 0 ? 'dimmed' : undefined}
                  />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
      <HistoryPagination total={total} page={page} onChange={setPage} />
    </>
  );
}

function QuoteHistory({ customer, canCreate }: { customer: Customer; canCreate: boolean }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const [page, setPage] = useState(1);
  const quotes = useCustomerQuotes(business.id, customer.id, page);
  if (quotes.isPending) return <ListSkeleton rows={3} height={40} />;
  if (quotes.isError) return <ErrorState error={quotes.error} onRetry={() => void quotes.refetch()} />;
  const { items, total } = quotes.data;
  if (items.length === 0) {
    return (
      <EmptyBlock
        compact
        icon={FileText}
        title="No quotes yet"
        action={
          canCreate && (
            <Button component={Link} to={`/quotes/new?customer=${customer.id}`} variant="light" leftSection={<FilePlus size={16} />}>
              New quote
            </Button>
          )
        }
      />
    );
  }
  return (
    <>
      <Table.ScrollContainer minWidth={560}>
        <Table highlightOnHover verticalSpacing="sm">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Number</Table.Th>
              <Table.Th>Date</Table.Th>
              <Table.Th>Expires</Table.Th>
              <Table.Th>Status</Table.Th>
              <Table.Th ta="right">Total</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((quote) => (
              <Table.Tr key={quote.id} {...rowProps(`/quotes/${quote.id}`, navigate)}>
                <Table.Td>
                  <Anchor component={Link} to={`/quotes/${quote.id}`} fw={500} size="sm" c="inherit">
                    {quote.number}
                  </Anchor>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{formatDate(quote.issue_date)}</Text>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{formatDate(quote.expiry_date)}</Text>
                </Table.Td>
                <Table.Td>
                  <StatusBadge status={quote.display_status} />
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={quote.total} currency={quote.currency} size="sm" />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
      <HistoryPagination total={total} page={page} onChange={setPage} />
    </>
  );
}

// ─── Page ───────────────────────────────────────────────────────────────────

function CustomerView({ customerId }: { customerId: string }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const customer = useCustomer(business.id, customerId);
  const [tab, setTab] = useState<string | null>('invoices');
  const [editing, editModal] = useDisclosure(false);
  const [statementOpen, statementModal] = useDisclosure(false);
  const emailEnabled = useEmailEnabled();
  const followUp = [businessKeys.dashboard(business.id), businessKeys.activityAll(business.id)];

  const setActive = useApiMutation(
    ({ current, active }: { current: Customer; active: boolean }) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/customers/{customer_id}', {
          params: { path: { business_id: business.id, customer_id: current.id } },
          body: withActive(current, active),
        }),
      ),
    {
      successMessage: (saved) => (saved.is_active ? `${saved.name} reactivated` : `${saved.name} deactivated`),
      invalidate: [customerKeys.all(business.id), ...followUp],
    },
  );
  const remove = useApiMutation(
    (current: Customer) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/customers/{customer_id}', {
          params: { path: { business_id: business.id, customer_id: current.id } },
        }),
      ),
    {
      successMessage: (_, current) => `${current.name} deleted`,
      // Lists only: the open detail would refetch a customer that no longer exists.
      invalidate: [customerKeys.lists(business.id), ...followUp],
      onSuccess: () => {
        navigate('/customers', { replace: true });
        queryClient.removeQueries({ queryKey: customerKeys.detail(business.id, customerId) });
      },
    },
  );

  if (customer.isPending) {
    return (
      <Page title="Customer" eyebrow={backLink}>
        <ListSkeleton rows={5} height={72} />
      </Page>
    );
  }
  if (customer.isError) {
    const notFound = isApiError(customer.error) && customer.error.code === 'NOT_FOUND';
    return (
      <Page title="Customer" eyebrow={backLink}>
        {notFound ? (
          <EmptyBlock
            icon={UserX}
            title="Customer not found"
            description="It may have been deleted, or it belongs to another business."
            action={
              <Button component={Link} to="/customers" variant="default">
                Back to customers
              </Button>
            }
          />
        ) : (
          <ErrorState error={customer.error} onRetry={() => void customer.refetch()} />
        )}
      </Page>
    );
  }

  const data = customer.data;
  const active = data.is_active;
  const subtitle = [data.business_name !== data.name ? data.business_name : '', data.contact_person].filter(Boolean).join(' · ');

  const deactivate = async () => {
    const confirmed = await confirmAction({
      title: `Deactivate ${data.name}?`,
      message:
        'They’ll be hidden from customer lists and can’t be used on new invoices or quotes. Existing documents aren’t affected, and you can reactivate them at any time.',
      confirmLabel: 'Deactivate',
    });
    if (confirmed) setActive.mutate({ current: data, active: false });
  };
  const deleteCustomer = async () => {
    const confirmed = await confirmAction({
      title: `Delete ${data.name}?`,
      message:
        'This permanently deletes the customer. Customers with invoices, quotes or other records can’t be deleted: deactivate them instead.',
      confirmLabel: 'Delete customer',
      danger: true,
    });
    if (confirmed) remove.mutate(data);
  };

  const actions = (
    <>
      {active ? (
        <>
          <Button component={Link} to={`/invoices/new?customer=${data.id}`} leftSection={<FilePlus size={16} />}>
            New invoice
          </Button>
          <Button component={Link} to={`/quotes/new?customer=${data.id}`} variant="default" leftSection={<FileText size={16} />}>
            New quote
          </Button>
        </>
      ) : (
        <Button
          leftSection={<RotateCcw size={16} />}
          loading={setActive.isPending}
          onClick={() => setActive.mutate({ current: data, active: true })}
        >
          Reactivate
        </Button>
      )}
      <Button
        component={Link}
        to={`/reports?report=statement_of_account&customer=${data.id}`}
        variant="default"
        leftSection={<ScrollText size={16} />}
      >
        Statement
      </Button>

      <Button variant="default" leftSection={<Pencil size={16} />} onClick={editModal.open}>
        Edit
      </Button>
      <Menu position="bottom-end" withinPortal>
        <Menu.Target>
          <ActionIcon variant="default" size="lg" aria-label="More actions" loading={remove.isPending || (active && setActive.isPending)}>
            <EllipsisVertical size={18} />
          </ActionIcon>
        </Menu.Target>
        <Menu.Dropdown>
          {emailEnabled && (
            <Menu.Item leftSection={<Mail size={14} />} onClick={statementModal.open}>
              Email statement
            </Menu.Item>
          )}
          {active && (
            <Menu.Item leftSection={<Power size={14} />} onClick={() => void deactivate()}>
              Deactivate
            </Menu.Item>
          )}
          <Menu.Item color="red" leftSection={<Trash2 size={14} />} onClick={() => void deleteCustomer()}>
            Delete
          </Menu.Item>
        </Menu.Dropdown>
      </Menu>
    </>
  );

  return (
    <Page
      eyebrow={backLink}
      title={
        <>
          {data.name}{' '}
          {!active && (
            <Badge component="span" color="gray" variant="light" size="lg" style={{ verticalAlign: 'middle' }}>
              Inactive
            </Badge>
          )}
        </>
      }
      description={subtitle || undefined}
      actions={actions}
    >
      {!active && (
        <Alert color="gray" variant="light">
          This customer is inactive: hidden from customer lists and not available for new invoices or quotes.
        </Alert>
      )}
      <Summary customer={data} currency={business.currency} />
      <ContactDetails customer={data} />
      <SectionCard>
        <CustomerRemindersSwitch customerId={data.id} sendReminders={data.send_reminders} />
      </SectionCard>
      {data.notes && (
        <SectionCard title="Notes">
          <Text size="sm" style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere' }}>
            {data.notes}
          </Text>
        </SectionCard>
      )}
      <SectionCard padding="md">
        <Tabs value={tab} onChange={setTab} keepMounted={false}>
          <Tabs.List mb="md">
            <Tabs.Tab value="invoices">Invoices</Tabs.Tab>
            <Tabs.Tab value="quotes">Quotes</Tabs.Tab>
          </Tabs.List>
          <Tabs.Panel value="invoices">
            <InvoiceHistory customer={data} canCreate={active} />
          </Tabs.Panel>
          <Tabs.Panel value="quotes">
            <QuoteHistory customer={data} canCreate={active} />
          </Tabs.Panel>
        </Tabs>
      </SectionCard>
      <StatementEmailsCard customerId={data.id} />
      <CustomerFormModal opened={editing} onClose={editModal.close} customer={data} />
      <EmailStatementModal opened={statementOpen} onClose={statementModal.close} customer={data} />
    </Page>
  );
}

export function CustomerDetailPage() {
  const { customerId = '' } = useParams();
  // Keyed so moving between customers starts with fresh tabs and pages.
  return <CustomerView key={customerId} customerId={customerId} />;
}
