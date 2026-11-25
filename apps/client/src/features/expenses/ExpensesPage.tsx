import {
  Badge,
  Box,
  Button,
  Group,
  Pagination,
  Paper,
  Select,
  Stack,
  Table,
  Text,
  TextInput,
  Tooltip,
  UnstyledButton,
  VisuallyHidden,
} from '@mantine/core';
import { useDebouncedValue } from '@mantine/hooks';
import type { Business } from '@solo/api-client';
import { Paperclip, Plus, Receipt, Search } from 'lucide-react';
import { useState } from 'react';
import { useSearchParams } from 'react-router';
import { MoneyText } from '@/components/Money';
import { Page } from '@/components/Page';
import { PeriodPicker } from '@/components/PeriodPicker';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { formatDate } from '@/lib/format';
import { periodRange, type Period } from '@/lib/periods';
import { useExpenseCategories, useExpenses, type Expense, type ExpenseListParams } from './api';
import { ExpenseDetailDrawer } from './ExpenseDetailDrawer';
import { CLASSIFICATION_LABELS, CLASSIFICATIONS } from './expenseForm';
import { ExpenseFormModal } from './ExpenseFormModal';
import { formatShare } from './expenseMath';

const PER_PAGE = 25;

type ReceiptFilter = 'any' | 'with' | 'missing';

const RECEIPT_OPTIONS: { value: ReceiptFilter; label: string }[] = [
  { value: 'any', label: 'Any receipt' },
  { value: 'with', label: 'With receipt' },
  { value: 'missing', label: 'Missing receipt' },
];

const CLASSIFICATION_OPTIONS = CLASSIFICATIONS.map((value) => ({ value, label: CLASSIFICATION_LABELS[value] }));

function ReceiptIcon({ count }: { count: number }) {
  if (count === 0) return null;
  const label = `${count} receipt${count === 1 ? '' : 's'} attached`;
  return (
    <Tooltip label={label}>
      <Box component="span" c="dimmed" style={{ display: 'inline-flex' }} aria-label={label} role="img">
        <Paperclip size={15} />
      </Box>
    </Tooltip>
  );
}

function BusinessShare({ expense }: { expense: Expense }) {
  if (expense.classification === 'personal') {
    return (
      <Badge color="gray" variant="light">
        Personal
      </Badge>
    );
  }
  return (
    <Group gap={6} justify="flex-end" wrap="nowrap">
      {expense.classification === 'rechargeable' && <Badge color="blue">Recharge</Badge>}
      <Text size="sm" className="tabular">
        {formatShare(expense.business_percentage)}
      </Text>
    </Group>
  );
}

function ExpenseTable({ items, onOpen }: { items: Expense[]; onOpen: (id: string) => void }) {
  return (
    <Paper withBorder radius="lg" style={{ overflow: 'hidden' }}>
      <Table.ScrollContainer minWidth={820}>
        <Table>
          <Table.Thead>
            <Table.Tr>
              <Table.Th w={110}>Date</Table.Th>
              <Table.Th>Supplier</Table.Th>
              <Table.Th>Category</Table.Th>
              <Table.Th ta="right">Amount</Table.Th>
              <Table.Th ta="right">GST</Table.Th>
              <Table.Th ta="right">Business share</Table.Th>
              <Table.Th w={40}>
                <VisuallyHidden>Receipt</VisuallyHidden>
              </Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {items.map((expense) => (
              <Table.Tr key={expense.id} onClick={() => onOpen(expense.id)} style={{ cursor: 'pointer' }}>
                <Table.Td style={{ whiteSpace: 'nowrap' }}>{formatDate(expense.expense_date)}</Table.Td>
                <Table.Td maw={320}>
                  {/* Focusable for keyboard users; its click bubbles to the row. */}
                  <UnstyledButton display="block" maw="100%">
                    <Text size="sm" fw={500} truncate="end">
                      {expense.supplier}
                    </Text>
                    {expense.description && (
                      <Text size="xs" c="dimmed" truncate="end">
                        {expense.description}
                      </Text>
                    )}
                  </UnstyledButton>
                </Table.Td>
                <Table.Td>
                  <Text size="sm">{expense.category_name}</Text>
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={expense.gross_amount} currency={expense.currency} size="sm" fw={500} />
                </Table.Td>
                <Table.Td ta="right">
                  {expense.gst_amount > 0 ? (
                    <MoneyText value={expense.gst_amount} currency={expense.currency} size="sm" c="dimmed" />
                  ) : (
                    <Text size="sm" c="dimmed">
                      —
                    </Text>
                  )}
                </Table.Td>
                <Table.Td ta="right">
                  <BusinessShare expense={expense} />
                </Table.Td>
                <Table.Td>
                  <ReceiptIcon count={expense.attachment_count} />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
    </Paper>
  );
}

function ExpenseCards({ items, onOpen }: { items: Expense[]; onOpen: (id: string) => void }) {
  return (
    <Stack gap="xs">
      {items.map((expense) => (
        <UnstyledButton key={expense.id} onClick={() => onOpen(expense.id)}>
          <Paper withBorder radius="md" p="sm">
            <Group justify="space-between" wrap="nowrap" gap="sm" align="flex-start">
              <Box style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} truncate="end">
                  {expense.supplier}
                </Text>
                <Text size="xs" c="dimmed" truncate="end">
                  {formatDate(expense.expense_date)} · {expense.category_name}
                </Text>
              </Box>
              <Stack gap={2} align="flex-end" style={{ flexShrink: 0 }}>
                <MoneyText value={expense.gross_amount} currency={expense.currency} size="sm" fw={600} />
                <Group gap={6} wrap="nowrap">
                  <ReceiptIcon count={expense.attachment_count} />
                  {expense.classification !== 'business' && <BusinessShare expense={expense} />}
                </Group>
              </Stack>
            </Group>
            {expense.description && (
              <Text size="xs" c="dimmed" mt={4} lineClamp={2}>
                {expense.description}
              </Text>
            )}
          </Paper>
        </UnstyledButton>
      ))}
    </Stack>
  );
}

function ExpensesView({ business }: { business: Business }) {
  const isMobile = useIsMobile();
  const [searchParams, setSearchParams] = useSearchParams();
  const creating = searchParams.get('new') === '1';
  const selectedId = searchParams.get('expense');
  const updateParams = (change: (next: URLSearchParams) => void) =>
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        change(next);
        return next;
      },
      { replace: true },
    );
  const openExpense = (id: string) => updateParams((p) => p.set('expense', id));

  const [period, setPeriod] = useState<Period>(() => periodRange('this_fy', business.today, business.current_financial_year.start));
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [debouncedSearch] = useDebouncedValue(search.trim(), 300);
  const [receipt, setReceipt] = useState<ReceiptFilter>('any');
  const [classification, setClassification] = useState<string | null>(null);
  const [page, setPage] = useState(1);

  const params: ExpenseListParams = {
    from: period.from,
    to: period.to,
    category_id: categoryId ?? undefined,
    search: debouncedSearch || undefined,
    has_receipt: receipt === 'any' ? undefined : receipt === 'with',
    classification: classification ?? undefined,
    page,
    per_page: PER_PAGE,
  };
  const expenses = useExpenses(params);
  const categories = useExpenseCategories();
  const categoryOptions = (categories.data ?? []).map((c) => ({ value: c.id, label: c.archived ? `${c.name} (archived)` : c.name }));

  const filtered = categoryId !== null || search.trim() !== '' || receipt !== 'any' || classification !== null;
  const clearFilters = () => {
    setCategoryId(null);
    setSearch('');
    setReceipt('any');
    setClassification(null);
    setPage(1);
  };

  const addButton = (
    <Button leftSection={<Plus size={16} />} onClick={() => updateParams((p) => p.set('new', '1'))}>
      Add expense
    </Button>
  );

  let content;
  if (expenses.isPending) {
    content = <ListSkeleton rows={8} height={isMobile ? 72 : 48} />;
  } else if (expenses.isError) {
    content = <ErrorState error={expenses.error} onRetry={() => void expenses.refetch()} />;
  } else {
    const { items, total, total_gross, total_gst } = expenses.data;
    content =
      items.length === 0 ? (
        filtered ? (
          <EmptyBlock
            icon={Search}
            title="No expenses match these filters"
            description="Try a different search or clear the filters."
            action={
              <Button variant="default" onClick={clearFilters}>
                Clear filters
              </Button>
            }
          />
        ) : (
          <EmptyBlock
            icon={Receipt}
            title="No expenses in this period"
            description="Record what you spend on the business, with receipts, so you can claim the GST and the cost at tax time."
            action={addButton || undefined}
          />
        )
      ) : (
        <Stack gap="sm" style={{ opacity: expenses.isPlaceholderData ? 0.6 : 1, transition: 'opacity 120ms' }}>
          <Group gap="lg" wrap="wrap">
            <Text size="sm" c="dimmed">
              {total} {total === 1 ? 'expense' : 'expenses'}
            </Text>
            <Text size="sm">
              Total spent <MoneyText value={total_gross} currency={business.currency} fw={600} />
            </Text>
            {(business.gst_registered || total_gst > 0) && (
              <Text size="sm">
                GST included <MoneyText value={total_gst} currency={business.currency} fw={600} />
              </Text>
            )}
          </Group>
          {isMobile ? <ExpenseCards items={items} onOpen={openExpense} /> : <ExpenseTable items={items} onOpen={openExpense} />}
          {total > PER_PAGE && (
            <Group justify="flex-end">
              <Pagination total={Math.ceil(total / PER_PAGE)} value={page} onChange={setPage} size="sm" />
            </Group>
          )}
        </Stack>
      );
  }

  return (
    <Page title="Expenses" description="What you’ve spent on the business, with receipts and GST." actions={addButton}>
      <Group gap="xs" wrap="wrap" align="flex-start">
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
          aria-label="Search expenses"
          placeholder="Search supplier, description or reference"
          leftSection={<Search size={15} />}
          w={{ base: '100%', xs: 300 }}
          value={search}
          onChange={(event) => {
            setSearch(event.currentTarget.value);
            setPage(1);
          }}
        />
        <Select
          aria-label="Category"
          placeholder="All categories"
          clearable
          searchable
          w={{ base: '100%', xs: 200 }}
          data={categoryOptions}
          value={categoryId}
          onChange={(value) => {
            setCategoryId(value);
            setPage(1);
          }}
        />
        <Select
          aria-label="Receipt"
          w={{ base: '100%', xs: 170 }}
          data={RECEIPT_OPTIONS}
          value={receipt}
          onChange={(value) => {
            if (!value) return;
            setReceipt(value);
            setPage(1);
          }}
        />
        <Select
          aria-label="Type"
          placeholder="All types"
          clearable
          w={{ base: '100%', xs: 200 }}
          data={CLASSIFICATION_OPTIONS}
          value={classification}
          onChange={(value) => {
            setClassification(value);
            setPage(1);
          }}
        />
      </Group>
      {content}
      <ExpenseFormModal
        opened={creating}
        onClose={() => updateParams((p) => p.delete('new'))}
        onSaved={(saved) =>
          updateParams((p) => {
            p.delete('new');
            p.set('expense', saved.id);
          })
        }
      />
      <ExpenseDetailDrawer expenseId={selectedId} onClose={() => updateParams((p) => p.delete('expense'))} />
    </Page>
  );
}

/** Expenses for the current business. `?new=1` opens the add form; `?expense=<id>` opens an expense. */
export function ExpensesPage() {
  const business = useCurrentBusiness();
  return <ExpensesView key={business.id} business={business} />;
}
