import { ActionIcon, Anchor, Badge, Button, Group, Menu, Modal, Paper, SimpleGrid, Stack, Table, Text, Tooltip } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import type { components } from '@solo/api-client';
import {
  ArrowLeft,
  Ban,
  Repeat,
  Check,
  ChevronDown,
  Download,
  ExternalLink,
  FileCheck,
  Mail,
  MoreHorizontal,
  Pencil,
  Send,
  Trash2,
  Wallet,
  X,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { Link, useNavigate, useParams } from 'react-router';
import { confirmAction } from '@/components/confirmAction';
import { MoneyText, StatusBadge } from '@/components/Money';
import { Page } from '@/components/Page';
import { promptReason } from '@/components/promptReason';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useEmailEnabled } from '@/features/server/api';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate, formatDateTime } from '@/lib/format';
import { api, fetchBlob, unwrap } from '@/services/api/client';
import { errorMessage } from '@/services/api/errors';
import { useApiMutation } from '@/services/api/query';
import { isTauri, saveBlob } from '@/services/platform';
import { financialKeys, useInvoice, useQuote, type Invoice, type Quote } from './api';
import { PaymentForm, SendForm } from './DocumentModals';
import { RepeatInvoiceModal } from './Recurring';
import { paymentMethodLabel, taxLabel } from './labels';

type Payment = components['schemas']['PaymentRow'];
type Journal = components['schemas']['JournalEntryView'];

function Lines({ doc }: { doc: Invoice | Quote }) {
  const gst = doc.lines.some((l) => l.tax_treatment !== 'no_gst');
  return (
    <SectionCard padding="lg">
      <Stack gap="md">
        <Group justify="space-between" align="flex-start">
          <div>
            <Text size="xs" c="dimmed" fw={600} tt="uppercase">
              {'due_date' in doc ? 'Bill to' : 'Prepared for'}
            </Text>
            <Text fw={600}>{doc.customer.name}</Text>
            {doc.customer.business_name && doc.customer.business_name !== doc.customer.name && (
              <Text size="sm">{doc.customer.business_name}</Text>
            )}
            {doc.customer.email && (
              <Text size="sm" c="dimmed">
                {doc.customer.email}
              </Text>
            )}
          </div>
          <Stack gap={2} align="flex-end">
            <Text size="sm">
              <Text span c="dimmed">
                {'due_date' in doc ? 'Issued ' : 'Dated '}
              </Text>
              {formatDate(doc.issue_date)}
            </Text>
            <Text size="sm">
              <Text span c="dimmed">
                {'due_date' in doc ? 'Due ' : 'Valid until '}
              </Text>
              {formatDate('due_date' in doc ? doc.due_date : doc.expiry_date)}
            </Text>
            {doc.reference && (
              <Text size="sm">
                <Text span c="dimmed">
                  Ref{' '}
                </Text>
                {doc.reference}
              </Text>
            )}
          </Stack>
        </Group>
        <Table.ScrollContainer minWidth={560}>
          <Table verticalSpacing="xs">
            <Table.Thead>
              <Table.Tr>
                <Table.Th>Description</Table.Th>
                <Table.Th ta="right">Qty</Table.Th>
                <Table.Th ta="right">{doc.prices_include_gst ? 'Price incl. GST' : 'Price'}</Table.Th>
                <Table.Th ta="right">Discount</Table.Th>
                {gst && <Table.Th ta="right">GST</Table.Th>}
                <Table.Th ta="right">Amount</Table.Th>
              </Table.Tr>
            </Table.Thead>
            <Table.Tbody>
              {doc.lines.map((l) => (
                <Table.Tr key={l.id}>
                  <Table.Td style={{ whiteSpace: 'pre-wrap' }}>{l.description}</Table.Td>
                  <Table.Td ta="right" className="tabular">
                    {Number(l.quantity)}
                  </Table.Td>
                  <Table.Td ta="right">
                    <MoneyText value={l.unit_price} currency={doc.currency} size="sm" />
                  </Table.Td>
                  <Table.Td ta="right">{Number(l.discount_percent) ? `${Number(l.discount_percent)}%` : '—'}</Table.Td>
                  {gst && <Table.Td ta="right">{taxLabel(l.tax_treatment, l.tax_rate)}</Table.Td>}
                  <Table.Td ta="right">
                    <MoneyText value={l.net_amount} currency={doc.currency} size="sm" />
                  </Table.Td>
                </Table.Tr>
              ))}
            </Table.Tbody>
          </Table>
        </Table.ScrollContainer>
        <Group justify="flex-end">
          <Stack gap={4} w={260}>
            <Group justify="space-between">
              <Text c="dimmed" size="sm">
                Subtotal
              </Text>
              <MoneyText value={doc.subtotal} currency={doc.currency} size="sm" />
            </Group>
            {gst && (
              <Group justify="space-between">
                <Text c="dimmed" size="sm">
                  GST
                </Text>
                <MoneyText value={doc.gst_total} currency={doc.currency} size="sm" />
              </Group>
            )}
            <Group justify="space-between">
              <Text fw={700}>Total</Text>
              <MoneyText value={doc.total} currency={doc.currency} fw={700} />
            </Group>
          </Stack>
        </Group>
        {doc.notes && (
          <div>
            <Text size="xs" c="dimmed" fw={600} tt="uppercase">
              Notes
            </Text>
            <Text size="sm" style={{ whiteSpace: 'pre-wrap' }}>
              {doc.notes}
            </Text>
          </div>
        )}
      </Stack>
    </SectionCard>
  );
}

function JournalCard({ journal, currency }: { journal: Journal[]; currency: string }) {
  if (journal.length === 0) return null;
  return (
    <SectionCard
      title="In your books"
      description="Entries posted to your ledger for this record. Corrections appear as reversals, never edits."
    >
      <Stack gap={6}>
        {journal.map((e) => (
          <Group key={e.id} justify="space-between" wrap="nowrap" gap="xs">
            <div style={{ minWidth: 0 }}>
              <Text size="sm" className="clamp-1">
                {e.description}
              </Text>
              <Text size="xs" c="dimmed">
                {formatDate(e.entry_date)} · {e.basis === 'both' ? 'cash and accrual' : `${e.basis} basis`}
              </Text>
            </div>
            <MoneyText value={e.amount} currency={currency} size="sm" c={e.reverses_entry_id ? 'red' : undefined} />
          </Group>
        ))}
      </Stack>
    </SectionCard>
  );
}

function EmailsCard({ emails }: { emails: components['schemas']['EmailLogRow'][] }) {
  if (emails.length === 0) return null;
  return (
    <SectionCard title="Emails">
      <Stack gap={6}>
        {emails.map((e) => (
          <Group key={e.id} justify="space-between" wrap="nowrap">
            <div style={{ minWidth: 0 }}>
              <Text size="sm" className="clamp-1">
                {e.subject}
              </Text>
              <Text size="xs" c="dimmed" className="clamp-1">
                {e.to_email} · {formatDateTime(e.created_at)}
              </Text>
            </div>
            <Tooltip label={e.error ?? ''} disabled={!e.error}>
              <Badge color={e.status === 'sent' ? 'teal' : 'red'} style={{ flexShrink: 0 }}>
                {e.status === 'sent' ? 'Sent' : 'Failed'}
              </Badge>
            </Tooltip>
          </Group>
        ))}
      </Stack>
    </SectionCard>
  );
}

/** Download the PDF; on the web also offer to open it in a new tab. */
function PdfButtons({ path, filename }: { path: string; filename: string }) {
  const [busy, setBusy] = useState(false);
  const get = async (open: boolean) => {
    setBusy(true);
    try {
      const blob = await fetchBlob(path);
      if (open) window.open(URL.createObjectURL(blob), '_blank', 'noopener');
      else saveBlob(blob, filename);
    } catch (error) {
      notifications.show({ color: 'red', title: 'PDF unavailable', message: errorMessage(error) });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Group gap={0}>
      <Button
        variant="default"
        leftSection={<Download size={15} />}
        loading={busy}
        onClick={() => void get(false)}
        style={!isTauri() ? { borderTopRightRadius: 0, borderBottomRightRadius: 0 } : undefined}
      >
        PDF
      </Button>
      {!isTauri() && (
        <Tooltip label="Open PDF in a new tab">
          <ActionIcon
            variant="default"
            size={36}
            aria-label="Open PDF in a new tab"
            disabled={busy}
            onClick={() => void get(true)}
            style={{ borderTopLeftRadius: 0, borderBottomLeftRadius: 0, borderLeft: 0 }}
          >
            <ExternalLink size={15} />
          </ActionIcon>
        </Tooltip>
      )}
    </Group>
  );
}

function Back({ to, label }: { to: string; label: string }) {
  return (
    <Anchor component={Link} to={to} size="sm" c="dimmed">
      <Group gap={4}>
        <ArrowLeft size={14} /> {label}
      </Group>
    </Anchor>
  );
}

function Summary({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <Paper withBorder radius="lg" p="md" bg="var(--app-surface-raised)">
      <SimpleGrid cols={{ base: 2, sm: rows.length }} spacing="md">
        {rows.map(([label, value]) => (
          <div key={label}>
            <Text size="xs" c="dimmed">
              {label}
            </Text>
            <Text fw={650} size="lg" className="tabular">
              {value}
            </Text>
          </div>
        ))}
      </SimpleGrid>
    </Paper>
  );
}

// ─── Invoice ────────────────────────────────────────────────────────────────

function InvoiceView({ invoice }: { invoice: Invoice }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const [paymentOpen, payment] = useDisclosure(false);
  const [editing, setEditing] = useState<Payment | undefined>(undefined);
  const [writingOff, setWritingOff] = useState(false);
  const [repeating, setRepeating] = useState(false);
  const [sendOpen, send] = useDisclosure(false);
  // Without email (the desktop app's own server), the PDF is downloaded and sent by the user.
  const emailEnabled = useEmailEnabled();
  const path = { business_id: business.id, invoice_id: invoice.id };
  const options = { invalidate: financialKeys(business.id) };
  const issue = useApiMutation(
    () => unwrap(api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/issue', { params: { path } })),
    {
      ...options,
      successMessage: (i) => `Invoice ${i.number} issued`,
    },
  );
  const markSent = useApiMutation(
    () => unwrap(api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/mark-sent', { params: { path } })),
    {
      ...options,
      successMessage: 'Marked as sent',
    },
  );
  const cancel = useApiMutation(
    (reason: string) =>
      unwrap(api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/cancel', { params: { path }, body: { reason } })),
    {
      ...options,
      successMessage: 'Invoice cancelled',
    },
  );
  const remove = useApiMutation(() => unwrap(api.DELETE('/api/v1/businesses/{business_id}/invoices/{invoice_id}', { params: { path } })), {
    ...options,
    successMessage: 'Draft deleted',
    onSuccess: () => navigate('/invoices'),
  });
  const voidPayment = useApiMutation(
    ({ id, reason }: { id: string; reason: string }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/invoices/{invoice_id}/payments/{payment_id}', {
          params: { path: { ...path, payment_id: id } },
          body: { reason },
        }),
      ),
    { ...options, successMessage: 'Payment removed' },
  );
  const draft = invoice.status === 'draft';
  const issued = invoice.status === 'issued';
  const activePayments = invoice.payments.filter((p) => !p.voided_at);
  const number = invoice.number ?? 'Draft';

  const actions = (
    <Group gap="xs">
      {draft && (
        <>
          <Button variant="default" component={Link} to={`/invoices/${invoice.id}/edit`} leftSection={<Pencil size={15} />}>
            Edit
          </Button>
          <PdfButtons path={`/api/v1/businesses/${business.id}/invoices/${invoice.id}/pdf`} filename={`Invoice draft.pdf`} />
          <Button leftSection={<FileCheck size={15} />} loading={issue.isPending} onClick={() => issue.mutate(undefined)}>
            Issue invoice
          </Button>
        </>
      )}
      {!draft && <PdfButtons path={`/api/v1/businesses/${business.id}/invoices/${invoice.id}/pdf`} filename={`Invoice ${number}.pdf`} />}
      {issued && (
        <>
          {emailEnabled && (
            <Button variant="default" leftSection={<Mail size={15} />} onClick={send.open}>
              Send
            </Button>
          )}
          {invoice.balance > 0 && (
            <Button
              leftSection={<Wallet size={15} />}
              onClick={() => {
                setEditing(undefined);
                setWritingOff(false);
                payment.open();
              }}
            >
              Record payment
            </Button>
          )}
        </>
      )}
      {(draft || issued) && (
        <Menu>
          <Menu.Target>
            <ActionIcon variant="default" size={36} aria-label="More actions">
              <MoreHorizontal size={16} />
            </ActionIcon>
          </Menu.Target>
          <Menu.Dropdown>
            {issued && (
              <Menu.Item leftSection={<Pencil size={14} />} component={Link} to={`/invoices/${invoice.id}/edit`}>
                Edit invoice
              </Menu.Item>
            )}
            {issued && !invoice.sent_at && (
              <Menu.Item leftSection={<Send size={14} />} onClick={() => markSent.mutate(undefined)}>
                Mark as sent
              </Menu.Item>
            )}
            {invoice.status !== 'cancelled' && (
              <Menu.Item leftSection={<Repeat size={14} />} onClick={() => setRepeating(true)}>
                Repeat this invoice
              </Menu.Item>
            )}
            {issued && invoice.balance > 0 && (
              <Menu.Item
                leftSection={<Ban size={14} />}
                onClick={() => {
                  setEditing(undefined);
                  setWritingOff(true);
                  payment.open();
                }}
              >
                Write off as a bad debt
              </Menu.Item>
            )}
            {issued && (
              <Menu.Item
                leftSection={<X size={14} />}
                color="red"
                disabled={invoice.paid > 0}
                onClick={async () => {
                  const reason = await promptReason({
                    title: `Cancel invoice ${number}?`,
                    message: 'The invoice stays in your records marked as cancelled, and its entries are reversed in your books.',
                    label: 'Why is it cancelled?',
                    required: true,
                    confirmLabel: 'Cancel invoice',
                    danger: true,
                  });
                  if (reason) cancel.mutate(reason);
                }}
              >
                {invoice.paid > 0 ? 'Cancel (remove payments first)' : 'Cancel invoice'}
              </Menu.Item>
            )}
            {draft && (
              <Menu.Item
                leftSection={<Trash2 size={14} />}
                color="red"
                onClick={async () => {
                  if (
                    await confirmAction({
                      title: 'Delete this draft?',
                      message: 'Drafts aren’t in your books yet, so nothing else changes.',
                      confirmLabel: 'Delete',
                      danger: true,
                    })
                  ) {
                    remove.mutate(undefined);
                  }
                }}
              >
                Delete draft
              </Menu.Item>
            )}
          </Menu.Dropdown>
        </Menu>
      )}
    </Group>
  );

  return (
    <Page
      eyebrow={<Back to="/invoices" label="Invoices" />}
      title={
        <Group gap="sm" component="span">
          {invoice.number ? `Invoice ${invoice.number}` : 'Draft invoice'}
          <StatusBadge status={invoice.display_status} />
        </Group>
      }
      description={
        <Anchor component={Link} to={`/customers/${invoice.customer.id}`}>
          {invoice.customer.name}
        </Anchor>
      }
      actions={actions}
    >
      {invoice.status === 'cancelled' && (
        <Paper withBorder p="sm" radius="md" bg="var(--mantine-color-gray-light)">
          <Text size="sm">
            Cancelled {formatDateTime(invoice.cancelled_at)}: {invoice.cancellation_reason}
          </Text>
        </Paper>
      )}
      {!draft && (
        <Summary
          rows={[
            ['Total', <MoneyText key="t" value={invoice.total} currency={invoice.currency} />],
            ['Paid', <MoneyText key="p" value={invoice.paid} currency={invoice.currency} />],
            [
              'Balance',
              <MoneyText
                key="b"
                value={invoice.balance}
                currency={invoice.currency}
                c={invoice.display_status === 'overdue' ? 'red' : undefined}
              />,
            ],
            ['Due', formatDate(invoice.due_date)],
          ]}
        />
      )}
      <SimpleGrid cols={{ base: 1, lg: 3 }} spacing="lg">
        <div className="span-2-lg" style={{ minWidth: 0 }}>
          <Lines doc={invoice} />
        </div>
        <Stack gap="lg">
          {!draft && (
            <SectionCard title="Payments">
              {activePayments.length === 0 && invoice.payments.length === 0 ? (
                <EmptyBlock compact icon={Wallet} title="No payments yet" description="Record payments as you receive them." />
              ) : (
                <Stack gap="xs">
                  {invoice.payments.map((p) => (
                    <Group key={p.id} justify="space-between" wrap="nowrap" opacity={p.voided_at ? 0.55 : 1}>
                      <div style={{ minWidth: 0 }}>
                        <Text size="sm" td={p.voided_at ? 'line-through' : undefined}>
                          <MoneyText value={p.amount} currency={invoice.currency} size="sm" /> · {paymentMethodLabel(p.method)}
                        </Text>
                        <Text size="xs" c="dimmed" className="clamp-1">
                          {formatDate(p.payment_date)}
                          {p.reference ? ` · ${p.reference}` : ''}
                          {p.voided_at ? ` · removed${p.void_reason ? `: ${p.void_reason}` : ''}` : ''}
                        </Text>
                      </div>
                      {!p.voided_at && issued && (
                        <Group gap={2} wrap="nowrap">
                          <ActionIcon
                            aria-label="Edit payment"
                            onClick={() => {
                              setEditing(p);
                              setWritingOff(false);
                              payment.open();
                            }}
                          >
                            <Pencil size={14} />
                          </ActionIcon>
                          <ActionIcon
                            aria-label="Remove payment"
                            color="red"
                            onClick={async () => {
                              const reason = await promptReason({
                                title: 'Remove this payment?',
                                message: 'It stays in the history as removed, and its entries are reversed in your books.',
                                confirmLabel: 'Remove payment',
                                danger: true,
                              });
                              if (reason !== null) voidPayment.mutate({ id: p.id, reason });
                            }}
                          >
                            <Trash2 size={14} />
                          </ActionIcon>
                        </Group>
                      )}
                    </Group>
                  ))}
                </Stack>
              )}
            </SectionCard>
          )}
          <EmailsCard emails={invoice.emails} />
          <JournalCard journal={invoice.journal} currency={invoice.currency} />
        </Stack>
      </SimpleGrid>
      {repeating && <RepeatInvoiceModal invoice={invoice} opened onClose={() => setRepeating(false)} />}
      <Modal
        opened={paymentOpen}
        onClose={payment.close}
        title={editing ? 'Edit payment' : writingOff ? `Write off invoice ${number}` : 'Record payment'}
      >
        {paymentOpen && <PaymentForm invoice={invoice} payment={editing} writeOff={writingOff} onDone={payment.close} />}
      </Modal>
      <Modal opened={sendOpen} onClose={send.close} title={`Send invoice ${number}`} size="lg">
        {sendOpen && (
          <SendForm
            kind="invoice"
            id={invoice.id}
            number={number}
            customerEmail={invoice.customer.email}
            customerName={invoice.customer.name}
            onDone={send.close}
          />
        )}
      </Modal>
    </Page>
  );
}

export function InvoiceDetailPage() {
  const { invoiceId } = useParams();
  const business = useCurrentBusiness();
  const invoice = useInvoice(business.id, invoiceId);
  if (invoice.isPending) return <ListSkeleton rows={6} height={72} />;
  if (invoice.isError) return <ErrorState error={invoice.error} onRetry={() => void invoice.refetch()} />;
  return <InvoiceView invoice={invoice.data} />;
}

// ─── Quote ──────────────────────────────────────────────────────────────────

function QuoteView({ quote }: { quote: Quote }) {
  const business = useCurrentBusiness();
  const navigate = useNavigate();
  const [sendOpen, send] = useDisclosure(false);
  const emailEnabled = useEmailEnabled();
  const path = { business_id: business.id, quote_id: quote.id };
  const options = { invalidate: financialKeys(business.id) };
  const status = useApiMutation(
    (value: 'sent' | 'accepted' | 'rejected') =>
      unwrap(api.POST('/api/v1/businesses/{business_id}/quotes/{quote_id}/status', { params: { path }, body: { status: value } })),
    {
      ...options,
      successMessage: (_, value) => (value === 'accepted' ? 'Quote accepted' : value === 'rejected' ? 'Quote declined' : 'Marked as sent'),
    },
  );
  const convert = useApiMutation(
    () => unwrap(api.POST('/api/v1/businesses/{business_id}/quotes/{quote_id}/convert', { params: { path } })),
    {
      ...options,
      successMessage: 'Draft invoice created from the quote',
      onSuccess: (result) => navigate(`/invoices/${result.invoice_id}`),
    },
  );
  const remove = useApiMutation(() => unwrap(api.DELETE('/api/v1/businesses/{business_id}/quotes/{quote_id}', { params: { path } })), {
    ...options,
    successMessage: 'Quote deleted',
    onSuccess: () => navigate('/quotes'),
  });
  const open = quote.status === 'draft' || quote.status === 'sent';
  const decided = quote.status === 'accepted' || quote.status === 'rejected';
  const converted = !!quote.converted_invoice_id;
  return (
    <Page
      eyebrow={<Back to="/quotes" label="Quotes" />}
      title={
        <Group gap="sm" component="span">
          Quote {quote.number}
          <StatusBadge status={quote.display_status} />
        </Group>
      }
      description={
        <Anchor component={Link} to={`/customers/${quote.customer.id}`}>
          {quote.customer.name}
        </Anchor>
      }
      actions={
        <Group gap="xs">
          {!decided && !converted && (
            <Button variant="default" component={Link} to={`/quotes/${quote.id}/edit`} leftSection={<Pencil size={15} />}>
              Edit
            </Button>
          )}
          <PdfButtons path={`/api/v1/businesses/${business.id}/quotes/${quote.id}/pdf`} filename={`Quote ${quote.number}.pdf`} />
          {emailEnabled && !converted && quote.status !== 'rejected' && (
            <Button variant="default" leftSection={<Mail size={15} />} onClick={send.open}>
              Send
            </Button>
          )}
          {converted ? (
            <Button component={Link} to={`/invoices/${quote.converted_invoice_id}`}>
              View invoice
            </Button>
          ) : quote.status === 'accepted' ? (
            <Button loading={convert.isPending} onClick={() => convert.mutate(undefined)}>
              Create invoice
            </Button>
          ) : (
            open && (
              <Menu>
                <Menu.Target>
                  <Button rightSection={<ChevronDown size={14} />}>Customer’s answer</Button>
                </Menu.Target>
                <Menu.Dropdown>
                  <Menu.Item
                    leftSection={<Check size={14} />}
                    onClick={() => status.mutate('accepted')}
                    disabled={quote.display_status === 'expired'}
                  >
                    {quote.display_status === 'expired' ? 'Accepted (update the expiry first)' : 'Accepted'}
                  </Menu.Item>
                  <Menu.Item leftSection={<X size={14} />} onClick={() => status.mutate('rejected')}>
                    Declined
                  </Menu.Item>
                  {quote.status === 'draft' && (
                    <Menu.Item leftSection={<Send size={14} />} onClick={() => status.mutate('sent')}>
                      Mark as sent
                    </Menu.Item>
                  )}
                  <Menu.Divider />
                  <Menu.Item
                    leftSection={<Trash2 size={14} />}
                    color="red"
                    onClick={async () => {
                      if (
                        await confirmAction({
                          title: `Delete quote ${quote.number}?`,
                          message: 'Quotes aren’t in your books, so nothing else changes.',
                          confirmLabel: 'Delete',
                          danger: true,
                        })
                      ) {
                        remove.mutate(undefined);
                      }
                    }}
                  >
                    Delete quote
                  </Menu.Item>
                </Menu.Dropdown>
              </Menu>
            )
          )}
        </Group>
      }
    >
      <Summary
        rows={[
          ['Total', <MoneyText key="t" value={quote.total} currency={quote.currency} />],
          ['Valid until', formatDate(quote.expiry_date)],
          ['Sent', quote.sent_at ? formatDate(quote.sent_at) : 'Not yet'],
        ]}
      />
      <SimpleGrid cols={{ base: 1, lg: 3 }} spacing="lg">
        <div className="span-2-lg" style={{ minWidth: 0 }}>
          <Lines doc={quote} />
        </div>
        <Stack gap="lg">
          <EmailsCard emails={quote.emails} />
          {quote.emails.length === 0 && (
            <SectionCard title="Next steps">
              <Text size="sm" c="dimmed">
                Send the quote to your customer. When they accept, mark it accepted and turn it into an invoice in one click.
              </Text>
            </SectionCard>
          )}
        </Stack>
      </SimpleGrid>
      <Modal opened={sendOpen} onClose={send.close} title={`Send quote ${quote.number}`} size="lg">
        {sendOpen && (
          <SendForm
            kind="quote"
            id={quote.id}
            number={quote.number}
            customerEmail={quote.customer.email}
            customerName={quote.customer.name}
            onDone={send.close}
          />
        )}
      </Modal>
    </Page>
  );
}

export function QuoteDetailPage() {
  const { quoteId } = useParams();
  const business = useCurrentBusiness();
  const quote = useQuote(business.id, quoteId);
  if (quote.isPending) return <ListSkeleton rows={6} height={72} />;
  if (quote.isError) return <ErrorState error={quote.error} onRetry={() => void quote.refetch()} />;
  return <QuoteView quote={quote.data} />;
}
