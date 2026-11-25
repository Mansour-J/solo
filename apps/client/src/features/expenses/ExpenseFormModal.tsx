import {
  Alert,
  Button,
  Group,
  Modal,
  NumberInput,
  Paper,
  Radio,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  Textarea,
  TextInput,
} from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import type { Business, BusinessSettings } from '@solo/api-client';
import { Info } from 'lucide-react';
import { MoneyInput, MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { CustomerSelect } from '@/features/customers/CustomerSelect';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { expenseChangeKeys, useExpenseCategories, type Expense, type ExpenseCategory, type ExpenseDetail } from './api';
import {
  CLASSIFICATION_DESCRIPTIONS,
  CLASSIFICATION_LABELS,
  CLASSIFICATIONS,
  emptyExpenseValues,
  expenseSchema,
  fromExpense,
  GST_TREATMENT_OPTIONS,
  PAYMENT_METHOD_OPTIONS,
  toExpenseInput,
  type ExpenseFormValues,
} from './expenseForm';
import { businessShareHundredths, defaultDeductibleHundredths, gstRegisteredOn, hundredthsToString } from './expenseMath';
import { SplitSummary } from './SplitSummary';
import { useExpenseCalculation } from './useExpenseCalculation';

export interface ExpenseFormModalProps {
  opened: boolean;
  onClose: () => void;
  /** The expense to edit; omit to add a new one. */
  expense?: Expense | null;
  /** Called with the saved expense (after the modal closes). */
  onSaved?: (expense: ExpenseDetail) => void;
}

/** Add or edit an expense. */
export function ExpenseFormModal({ opened, onClose, expense, onSaved }: ExpenseFormModalProps) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={expense ? 'Edit expense' : 'Add expense'} size="lg" fullScreen={isMobile}>
      <ExpenseFormLoader key={expense?.id ?? 'new'} expense={expense ?? null} onClose={onClose} onSaved={onSaved} />
    </Modal>
  );
}

function ExpenseFormLoader({
  expense,
  onClose,
  onSaved,
}: {
  expense: Expense | null;
  onClose: () => void;
  onSaved?: (expense: ExpenseDetail) => void;
}) {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  const categories = useExpenseCategories();
  if (settings.isPending || categories.isPending) return <ListSkeleton rows={6} height={44} />;
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  if (categories.isError) return <ErrorState error={categories.error} onRetry={() => void categories.refetch()} />;
  return (
    <ExpenseForm
      business={business}
      settings={settings.data}
      categories={categories.data}
      expense={expense}
      onClose={onClose}
      onSaved={onSaved}
    />
  );
}

function gstNote(business: Business): string {
  return business.gst_registered && business.gst_registered_from
    ? `You weren’t registered for GST on this date (only from ${formatDate(business.gst_registered_from)}), so GST can’t be claimed. The full amount is recorded as the cost.`
    : 'You’re not registered for GST, so GST on this receipt can’t be claimed. The full amount is recorded as the cost.';
}

function ExpenseForm({
  business,
  settings,
  categories,
  expense,
  onClose,
  onSaved,
}: {
  business: Business;
  settings: BusinessSettings;
  categories: ExpenseCategory[];
  expense: Expense | null;
  onClose: () => void;
  onSaved?: (expense: ExpenseDetail) => void;
}) {
  const registeredOn = (date: string | null) => gstRegisteredOn(business, date);
  const form = useForm<ExpenseFormValues>({
    initialValues: expense
      ? fromExpense(expense, settings.default_deductible_percentage)
      : emptyExpenseValues(business.today, gstRegisteredOn(business, business.today)),
    validate: zodValidate<ExpenseFormValues>(expenseSchema(registeredOn)),
  });
  const save = useApiMutation(
    (values: ExpenseFormValues) => {
      const body = toExpenseInput(values, registeredOn(values.expense_date));
      return expense
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/expenses/{expense_id}', {
              params: { path: { business_id: business.id, expense_id: expense.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/expenses', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: expense ? 'Expense saved' : 'Expense added',
      invalidate: expenseChangeKeys(business.id),
      onSuccess: (saved) => {
        onClose();
        onSaved?.(saved);
      },
    },
  );

  const v = form.values;
  const currency = business.currency;
  const registered = registeredOn(v.expense_date);
  const standard = registered && v.gst_treatment === 'standard';
  const { calculation } = useExpenseCalculation(
    business.id,
    v.gross_amount && v.gross_amount > 0 && v.expense_date
      ? {
          expense_date: v.expense_date,
          gross_amount: v.gross_amount,
          gst_treatment: registered ? v.gst_treatment : 'no_gst',
          gst_amount: standard && v.gst_override ? v.gst_amount : null,
          classification: v.classification,
          business_percentage: v.classification === 'partial' ? String(v.business_percentage ?? '') || null : null,
        }
      : null,
  );
  const share = businessShareHundredths(v.classification, v.business_percentage);
  const deductibleDefault = defaultDeductibleHundredths(v.classification, share, settings.default_deductible_percentage);
  const categoryOptions = categories
    .filter((c) => !c.archived || c.id === v.category_id)
    .map((c) => ({ value: c.id, label: c.archived ? `${c.name} (archived)` : c.name }));

  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <DateInput label="Date" withAsterisk valueFormat={dateInputFormat()} {...form.getInputProps('expense_date')} />
          <MoneyInput
            label="Amount paid"
            description="The total on the receipt, including any GST."
            withAsterisk
            currency={currency}
            value={v.gross_amount}
            onChange={(amount) => form.setFieldValue('gross_amount', amount)}
            error={form.errors.gross_amount}
          />
          <TextInput label="Supplier" placeholder="Who you paid" withAsterisk maxLength={200} {...form.getInputProps('supplier')} />
          <Select
            label="Category"
            placeholder="Choose a category"
            withAsterisk
            searchable
            nothingFoundMessage="No matching categories"
            data={categoryOptions}
            {...form.getInputProps('category_id')}
          />
        </SimpleGrid>
        <TextInput label="Description" placeholder="What it was for" maxLength={1000} {...form.getInputProps('description')} />

        {registered ? (
          <Stack gap="xs">
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <Select label="GST" data={GST_TREATMENT_OPTIONS} {...form.getInputProps('gst_treatment')} />
              {standard && v.gst_override && (
                <MoneyInput
                  label="GST on the receipt"
                  currency={currency}
                  value={v.gst_amount}
                  onChange={(amount) => form.setFieldValue('gst_amount', amount)}
                  error={form.errors.gst_amount}
                />
              )}
            </SimpleGrid>
            {standard && (
              <Group justify="space-between" gap="sm">
                {!v.gst_override && (
                  <Text size="sm">
                    GST included: <MoneyText value={calculation?.gst_amount ?? 0} currency={currency} fw={600} />
                  </Text>
                )}
                <Switch
                  label="Enter GST from the receipt"
                  checked={v.gst_override}
                  onChange={(event) => {
                    const on = event.currentTarget.checked;
                    form.setFieldValue('gst_override', on);
                    if (on && v.gst_amount === null && calculation) form.setFieldValue('gst_amount', calculation.gst_amount);
                  }}
                />
              </Group>
            )}
          </Stack>
        ) : (
          <Alert color="gray" variant="light" icon={<Info size={16} />}>
            {gstNote(business)}
          </Alert>
        )}

        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <Select
            label="Paid with"
            data={PAYMENT_METHOD_OPTIONS}
            description={
              v.payment_method === 'personal_funds' ? 'The business share is recorded as money you’ve put into the business.' : undefined
            }
            {...form.getInputProps('payment_method')}
          />
          <TextInput
            label="Reference"
            placeholder="Receipt or transaction number"
            maxLength={100}
            {...form.getInputProps('payment_reference')}
          />
        </SimpleGrid>

        <Radio.Group label="Who was it for?" {...form.getInputProps('classification')}>
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="xs" mt="xs">
            {CLASSIFICATIONS.map((c) => (
              <Radio.Card key={c} value={c} p="sm" radius="md">
                <Group wrap="nowrap" align="flex-start" gap="sm">
                  <Radio.Indicator mt={2} />
                  <Stack gap={2}>
                    <Text size="sm" fw={500}>
                      {CLASSIFICATION_LABELS[c]}
                    </Text>
                    <Text size="xs" c="dimmed">
                      {CLASSIFICATION_DESCRIPTIONS[c]}
                    </Text>
                  </Stack>
                </Group>
              </Radio.Card>
            ))}
          </SimpleGrid>
        </Radio.Group>

        {v.classification !== 'personal' && (
          <SimpleGrid cols={{ base: 1, sm: 2 }}>
            {v.classification === 'partial' && (
              <NumberInput
                label="Business share"
                description="How much of this was for the business."
                withAsterisk
                suffix="%"
                min={0}
                max={100}
                decimalScale={2}
                {...form.getInputProps('business_percentage')}
              />
            )}
            {v.classification === 'rechargeable' && (
              <CustomerSelect
                label="Customer"
                description="Who you’ll recharge this to."
                withAsterisk
                value={v.rechargeable_customer_id}
                onChange={(id) => form.setFieldValue('rechargeable_customer_id', id)}
                error={form.errors.rechargeable_customer_id}
              />
            )}
            <NumberInput
              label="Deductible share"
              description={
                deductibleDefault === null
                  ? 'The share you’ll claim as a tax deduction.'
                  : `The share you’ll claim as a tax deduction. Leave empty to use ${hundredthsToString(deductibleDefault)}%.`
              }
              placeholder={deductibleDefault === null ? undefined : `${hundredthsToString(deductibleDefault)}%`}
              suffix="%"
              min={0}
              max={100}
              decimalScale={2}
              {...form.getInputProps('deductible_percentage')}
            />
          </SimpleGrid>
        )}

        {calculation && (v.classification !== 'partial' || share !== null) && (
          <Paper withBorder radius="md" p="sm">
            <Text size="xs" c="dimmed" mb={6}>
              How this will be recorded
            </Text>
            <SplitSummary split={calculation.split} currency={currency} paidPersonally={v.payment_method === 'personal_funds'} />
          </Paper>
        )}

        <Textarea label="Notes" maxLength={2000} {...form.getInputProps('notes')} />

        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {expense ? 'Save changes' : 'Add expense'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
