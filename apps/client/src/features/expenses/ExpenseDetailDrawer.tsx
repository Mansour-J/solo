import { Alert, Badge, Button, Drawer, Group, SimpleGrid, Stack, Table, Text, Title } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { Pencil, Trash2 } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { MoneyText } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate, formatMoney } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { expenseChangeKeys, useExpense, type ExpenseDetail, type PaymentMethod } from './api';
import { ExpenseAttachments } from './ExpenseAttachments';
import { CLASSIFICATION_LABELS, GST_TREATMENT_LABELS, PAYMENT_METHOD_LABELS } from './expenseForm';
import { ExpenseFormModal } from './ExpenseFormModal';
import { formatShare, type Classification } from './expenseMath';
import { SplitSummary } from './SplitSummary';

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Stack gap="xs">
      <Title order={4}>{title}</Title>
      {children}
    </Stack>
  );
}

function JournalHistory({ expense }: { expense: ExpenseDetail }) {
  const reversed = new Set(expense.journal.map((entry) => entry.reverses_entry_id).filter(Boolean));
  if (expense.journal.length === 0) {
    return (
      <Text size="sm" c="dimmed">
        No ledger entries.
      </Text>
    );
  }
  return (
    <Table.ScrollContainer minWidth={420}>
      <Table highlightOnHover={false}>
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Date</Table.Th>
            <Table.Th>Entry</Table.Th>
            <Table.Th ta="right">Amount</Table.Th>
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {expense.journal.map((entry) => (
            <Table.Tr key={entry.id}>
              <Table.Td style={{ whiteSpace: 'nowrap' }}>{formatDate(entry.entry_date)}</Table.Td>
              <Table.Td>
                <Group gap={6}>
                  <Text size="sm">{entry.description}</Text>
                  {entry.reverses_entry_id && <Badge color="orange">Reversal</Badge>}
                  {reversed.has(entry.id) && (
                    <Badge color="gray" variant="outline">
                      Reversed
                    </Badge>
                  )}
                </Group>
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText
                  value={entry.amount}
                  currency={expense.currency}
                  size="sm"
                  td={reversed.has(entry.id) ? 'line-through' : undefined}
                />
              </Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

function ExpenseDetailBody({ expense, onClose }: { expense: ExpenseDetail; onClose: () => void }) {
  const business = useCurrentBusiness();
  const [editing, edit] = useDisclosure(false);
  const remove = useApiMutation(
    (reason: string) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/expenses/{expense_id}', {
          params: { path: { business_id: business.id, expense_id: expense.id } },
          body: { reason },
        }),
      ),
    { successMessage: 'Expense deleted', invalidate: expenseChangeKeys(business.id), onSuccess: onClose },
  );

  const confirmDelete = async () => {
    const reason = await promptReason({
      title: 'Delete this expense?',
      message: 'It’s taken out of your books and reports, and its ledger entry is reversed. It stays in the history with your reason.',
      label: 'Reason (optional)',
      confirmLabel: 'Delete expense',
      danger: true,
    });
    if (reason !== null) remove.mutate(reason);
  };

  const classification = expense.classification as Classification;
  const rows: [string, ReactNode][] = [
    ['Date', formatDate(expense.expense_date)],
    ['Category', expense.category_name],
    [
      'Paid with',
      `${PAYMENT_METHOD_LABELS[expense.payment_method as PaymentMethod] ?? expense.payment_method}${expense.payment_reference ? ` · ${expense.payment_reference}` : ''}`,
    ],
    [
      'GST',
      `${GST_TREATMENT_LABELS[expense.gst_treatment]}${expense.gst_amount > 0 ? ` · ${formatMoney(expense.gst_amount, expense.currency)}` : ''}`,
    ],
    [
      'Used for',
      classification === 'partial'
        ? `${CLASSIFICATION_LABELS.partial} · ${formatShare(expense.business_percentage)} business`
        : (CLASSIFICATION_LABELS[classification] ?? expense.classification),
    ],
    ...(classification === 'rechargeable' ? ([['Recharge to', expense.rechargeable_customer_name ?? '—']] as [string, ReactNode][]) : []),
    ['Deductible share', formatShare(expense.deductible_percentage)],
  ];

  return (
    <Stack gap="lg">
      {expense.voided && (
        <Alert color="red" variant="light" title="Deleted">
          {expense.void_reason ? `Reason: ${expense.void_reason}. ` : ''}This expense no longer counts in your books.
        </Alert>
      )}
      <Group justify="space-between" align="flex-start" gap="sm">
        <Stack gap={2}>
          <MoneyText
            value={expense.gross_amount}
            currency={expense.currency}
            fz="1.5rem"
            fw={650}
            td={expense.voided ? 'line-through' : undefined}
          />
          {expense.description && <Text size="sm">{expense.description}</Text>}
        </Stack>
        {!expense.voided && (
          <Group gap="xs">
            <Button variant="default" leftSection={<Pencil size={14} />} onClick={edit.open}>
              Edit
            </Button>
            <Button
              variant="subtle"
              color="red"
              leftSection={<Trash2 size={14} />}
              loading={remove.isPending}
              onClick={() => void confirmDelete()}
            >
              Delete
            </Button>
          </Group>
        )}
      </Group>

      <SimpleGrid cols={{ base: 1, xs: 2 }} spacing="sm" verticalSpacing="sm">
        {rows.map(([label, value]) => (
          <Stack key={label} gap={2}>
            <Text size="xs" c="dimmed">
              {label}
            </Text>
            <Text size="sm">{value}</Text>
          </Stack>
        ))}
      </SimpleGrid>

      <Section title="How it’s recorded">
        <SplitSummary split={expense.split} currency={expense.currency} paidPersonally={expense.payment_method === 'personal_funds'} />
      </Section>

      {expense.notes && (
        <Section title="Notes">
          <Text size="sm" style={{ whiteSpace: 'pre-wrap' }}>
            {expense.notes}
          </Text>
        </Section>
      )}

      <Section title="Receipts">
        <ExpenseAttachments expense={expense} />
      </Section>

      <Section title="Ledger history">
        <JournalHistory expense={expense} />
      </Section>

      <ExpenseFormModal opened={editing} onClose={edit.close} expense={expense} />
    </Stack>
  );
}

/** Slide-out details of an expense: figures, receipts, ledger history, edit and delete. */
export function ExpenseDetailDrawer({ expenseId, onClose }: { expenseId: string | null; onClose: () => void }) {
  const isMobile = useIsMobile();
  // Keep showing the last expense while the drawer animates closed.
  const [shownId, setShownId] = useState(expenseId);
  if (expenseId && expenseId !== shownId) setShownId(expenseId);
  const expense = useExpense(shownId);
  return (
    <Drawer
      opened={!!expenseId}
      onClose={onClose}
      position="right"
      size={isMobile ? '100%' : 'lg'}
      title={<Text fw={650}>{expense.data?.supplier ?? 'Expense'}</Text>}
    >
      {expense.isPending ? (
        <ListSkeleton rows={6} height={48} />
      ) : expense.isError ? (
        <ErrorState error={expense.error} onRetry={() => void expense.refetch()} />
      ) : (
        <ExpenseDetailBody key={expense.data.id} expense={expense.data} onClose={onClose} />
      )}
    </Drawer>
  );
}
