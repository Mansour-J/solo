import { Button, Group, Modal, Select, SimpleGrid, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { Trash2 } from 'lucide-react';
import { MoneyInput } from '@/components/Money';
import { promptReason } from '@/components/promptReason';
import { allocationKeys } from '@/features/allocations/api';
import { offerAllocationReview } from '@/features/allocations/notify';
import { businessKeys, useCurrentBusiness } from '@/features/businesses/api';
import { CustomerSelect } from '@/features/customers/CustomerSelect';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate, formatMoney, minorToDecimalString } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { incomeKeys } from './api';
import {
  emptyIncomeForm,
  fromIncome,
  GST_TREATMENTS,
  gstApplies,
  gstPreview,
  INCOME_TYPES,
  incomeSchema,
  PAYMENT_METHODS,
  toIncomeInput,
  type IncomeFormValues,
  type IncomeRow,
} from './incomeForm';

function IncomeForm({ income, onDone }: { income: IncomeRow | null; onDone: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<IncomeFormValues>({
    initialValues: income ? fromIncome(income) : emptyIncomeForm(business, business.today),
    validate: zodValidate<IncomeFormValues>(incomeSchema(business)),
  });
  const invalidate = [
    incomeKeys.all(business.id),
    businessKeys.dashboard(business.id),
    businessKeys.activityAll(business.id),
    allocationKeys.all(business.id),
  ];
  const save = useApiMutation(
    (values: IncomeFormValues) => {
      const body = toIncomeInput(values, business);
      return income
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/income/{income_id}', {
              params: { path: { business_id: business.id, income_id: income.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/income', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: income ? 'Income updated' : 'Income recorded',
      invalidate,
      onSuccess: (saved) => {
        onDone();
        void offerAllocationReview(business.id, { income_id: saved.id });
      },
    },
  );
  const remove = useApiMutation(
    ({ id, reason }: { id: string; reason: string }) =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/income/{income_id}', {
          params: { path: { business_id: business.id, income_id: id } },
          body: { reason },
        }),
      ),
    { successMessage: 'Income deleted', invalidate, onSuccess: onDone },
  );

  const values = form.getValues();
  const registered = business.gst_registered;
  const gstActive = gstApplies(business, values.income_date);
  const standard = gstActive && values.gst_treatment === 'standard';
  const calculated = values.gross_amount !== null ? gstPreview(values.gross_amount) : null;
  const gst = standard ? (values.gst_amount ?? calculated) : 0;

  const confirmDelete = async (row: IncomeRow) => {
    const reason = await promptReason({
      title: 'Delete this income?',
      message: `Income of ${formatMoney(row.gross_amount, row.currency)} from ${row.payer} will be removed from your reports and GST. The record and your reason are kept in the history.`,
      confirmLabel: 'Delete income',
      danger: true,
    });
    if (reason !== null) remove.mutate({ id: row.id, reason });
  };

  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="md">
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <DateInput label="Date received" required valueFormat={dateInputFormat()} {...form.getInputProps('income_date')} />
          <MoneyInput
            label="Amount received"
            description={registered ? 'Including any GST.' : undefined}
            required
            currency={business.currency}
            data-autofocus
            {...form.getInputProps('gross_amount')}
            value={values.gross_amount}
          />
        </SimpleGrid>
        <TextInput label="Payer" description="Who paid you." required {...form.getInputProps('payer')} />
        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <Select
            label="Type"
            data={INCOME_TYPES}
            allowDeselect={false}
            description={values.income_type === 'other' ? 'Interest, grants and other non-sales income.' : 'Goods or services you sold.'}
            {...form.getInputProps('income_type')}
          />
          <CustomerSelect
            label="Customer"
            description="Optional."
            placeholder="No customer"
            clearable
            {...form.getInputProps('customer_id')}
            value={values.customer_id}
          />
        </SimpleGrid>

        {registered &&
          (gstActive ? (
            <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
              <Select label="GST" data={GST_TREATMENTS} allowDeselect={false} {...form.getInputProps('gst_treatment')} />
              {standard && (
                <MoneyInput
                  label="GST amount"
                  description={
                    calculated !== null
                      ? `Leave empty to calculate it (${formatMoney(calculated, business.currency)}).`
                      : 'Leave empty to calculate it.'
                  }
                  placeholder={calculated !== null ? minorToDecimalString(calculated, business.currency) : undefined}
                  currency={business.currency}
                  {...form.getInputProps('gst_amount')}
                  value={values.gst_amount}
                />
              )}
            </SimpleGrid>
          ) : (
            <Text size="sm" c="dimmed">
              No GST: this date is before your GST registration on {formatDate(business.gst_registered_from)}.
            </Text>
          ))}
        {standard && values.gross_amount !== null && gst !== null && (
          <Text size="sm" c="dimmed" className="tabular">
            GST {formatMoney(gst, business.currency)} · excluding GST {formatMoney(values.gross_amount - gst, business.currency)}
          </Text>
        )}

        <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
          <Select label="Payment method" data={PAYMENT_METHODS} allowDeselect={false} {...form.getInputProps('payment_method')} />
          <TextInput label="Reference" description="Optional, e.g. a receipt number." {...form.getInputProps('reference')} />
        </SimpleGrid>
        <TextInput label="Description" description="Optional: what the payment was for." {...form.getInputProps('description')} />
        <Textarea label="Notes" description="Private notes." autosize minRows={2} maxRows={6} {...form.getInputProps('notes')} />

        <Group justify="space-between" gap="sm">
          {income ? (
            <Button
              variant="subtle"
              color="red"
              leftSection={<Trash2 size={16} />}
              loading={remove.isPending}
              onClick={() => void confirmDelete(income)}
            >
              Delete
            </Button>
          ) : (
            <span />
          )}
          <Group gap="sm">
            <Button variant="default" onClick={onDone}>
              Cancel
            </Button>
            <Button type="submit" loading={save.isPending}>
              {income ? 'Save changes' : 'Record income'}
            </Button>
          </Group>
        </Group>
      </Stack>
    </form>
  );
}

/** Record (no `income`) or edit income received without an invoice. Fresh form on every open. */
export function IncomeFormModal({ opened, onClose, income = null }: { opened: boolean; onClose: () => void; income?: IncomeRow | null }) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={income ? 'Edit income' : 'Record income'} size="lg" fullScreen={isMobile}>
      <IncomeForm income={income} onDone={onClose} />
    </Modal>
  );
}
