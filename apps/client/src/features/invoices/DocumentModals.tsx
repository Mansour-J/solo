import { Button, Group, Select, SimpleGrid, Stack, Text, Textarea, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import type { components } from '@solo/api-client';
import { z } from 'zod';
import { MoneyInput } from '@/components/Money';
import { useCurrentUser } from '@/features/account/api';
import { offerAllocationReview } from '@/features/allocations/notify';
import { useCurrentBusiness } from '@/features/businesses/api';
import { dateInputFormat, formatMoney } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { financialKeys, type Invoice } from './api';
import { PAYMENT_METHODS } from './labels';

type Payment = components['schemas']['PaymentRow'];

interface PaymentValues {
  payment_date: string;
  amount: number | null;
  /** Tax the customer withheld and paid to IRD for you (schedular payments). */
  withholding_tax: number | null;
  method: components['schemas']['PaymentMethod'];
  reference: string;
  notes: string;
}

const paymentSchema = z
  .object({
    payment_date: z.string().min(1, 'Choose the date you were paid.'),
    amount: z.number({ error: 'Enter the amount received.' }).positive('Enter an amount above zero.'),
  })
  .passthrough();

/** Records a payment the user received (no money moves through the app), or edits one. */
export function PaymentForm({
  invoice,
  payment,
  writeOff = false,
  onDone,
}: {
  invoice: Invoice;
  payment?: Payment;
  /** Write off what's left as a bad debt instead of recording money received. */
  writeOff?: boolean;
  onDone: () => void;
}) {
  const business = useCurrentBusiness();
  const available = invoice.balance + (payment?.amount ?? 0);
  const form = useForm<PaymentValues>({
    initialValues: payment
      ? {
          payment_date: payment.payment_date,
          amount: payment.amount,
          withholding_tax: payment.withholding_tax || null,
          method: payment.method as PaymentValues['method'],
          reference: payment.reference,
          notes: payment.notes,
        }
      : {
          payment_date: business.today,
          amount: invoice.balance,
          withholding_tax: null,
          method: writeOff ? 'write_off' : 'bank_transfer',
          reference: '',
          notes: '',
        },
    validate: zodValidate<PaymentValues>(paymentSchema),
  });
  const path = { business_id: business.id, invoice_id: invoice.id };
  const save = useApiMutation(
    (v: PaymentValues) => {
      const body = { ...v, amount: v.amount ?? 0, withholding_tax: v.withholding_tax ?? 0 };
      return payment
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/invoices/{invoice_id}/payments/{payment_id}', {
              params: { path: { ...path, payment_id: payment.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/payments', { params: { path }, body }));
    },
    {
      form,
      invalidate: financialKeys(business.id),
      successMessage: payment ? 'Payment updated' : 'Payment recorded',
      onSuccess: (saved) => {
        onDone();
        // The payment just saved: the edited one, or the newest one recorded.
        const latest = [...saved.payments].sort((a, b) => b.created_at.localeCompare(a.created_at))[0];
        const paymentId = payment?.id ?? latest?.id;
        if (paymentId) void offerAllocationReview(business.id, { invoice_payment_id: paymentId });
      },
    },
  );
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack>
        {form.values.method === 'write_off' ? (
          <Text size="sm" c="dimmed">
            Write off what the customer won’t pay (up to {formatMoney(available, invoice.currency)}) as a bad debt. It becomes an expense
            and, on the invoice or hybrid GST basis, the GST in it comes off your GST return for the period you write it off. Keep a record
            of how you tried to collect it.
          </Text>
        ) : (
          <Text size="sm" c="dimmed">
            Record money you received for this invoice. Up to {formatMoney(available, invoice.currency)} is still owing.
          </Text>
        )}
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <DateInput label="Date received" valueFormat={dateInputFormat()} data-autofocus {...form.getInputProps('payment_date')} />
          <MoneyInput
            label="Amount"
            currency={invoice.currency}
            value={form.values.amount}
            onChange={(v) => form.setFieldValue('amount', v)}
            error={form.errors.amount}
          />
          {form.values.method !== 'write_off' && (
            <MoneyInput
              label="Tax withheld"
              description="If the customer deducted withholding tax, enter it: the amount is the invoice total it settles."
              currency={invoice.currency}
              value={form.values.withholding_tax}
              onChange={(v) => form.setFieldValue('withholding_tax', v)}
              error={form.errors.withholding_tax}
            />
          )}
          <Select label="Method" data={PAYMENT_METHODS} {...form.getInputProps('method')} />
          <TextInput label="Reference" placeholder="Bank reference" {...form.getInputProps('reference')} />
        </SimpleGrid>
        <Textarea label="Notes" autosize minRows={2} {...form.getInputProps('notes')} />
        <Group justify="flex-end">
          <Button variant="default" onClick={onDone}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {payment ? 'Save payment' : 'Record payment'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

interface SendValues {
  to: string;
  subject: string;
  message: string;
}

/** Emails an invoice or quote PDF to the customer. */
export function SendForm({
  kind,
  id,
  number,
  customerEmail,
  customerName,
  onDone,
}: {
  kind: 'invoice' | 'quote';
  id: string;
  number: string;
  customerEmail: string | null | undefined;
  customerName: string;
  onDone: () => void;
}) {
  const business = useCurrentBusiness();
  const user = useCurrentUser();
  const noun = kind === 'invoice' ? 'invoice' : 'quote';
  const form = useForm<SendValues>({
    initialValues: {
      to: customerEmail ?? '',
      subject: `${kind === 'invoice' ? 'Invoice' : 'Quote'} ${number} from ${business.name}`,
      message: `Hi ${customerName},\n\nPlease find ${noun} ${number} attached.\n\nThanks,\n${user.first_name || business.name}`,
    },
    validate: zodValidate<SendValues>(z.object({ to: z.string().trim().email('Enter the customer’s email address.') }).passthrough()),
  });
  const send = useApiMutation(
    async (v: SendValues): Promise<void> => {
      if (kind === 'invoice') {
        await unwrap(
          api.POST('/api/v1/businesses/{business_id}/invoices/{invoice_id}/send', {
            params: { path: { business_id: business.id, invoice_id: id } },
            body: v,
          }),
        );
      } else {
        await unwrap(
          api.POST('/api/v1/businesses/{business_id}/quotes/{quote_id}/send', {
            params: { path: { business_id: business.id, quote_id: id } },
            body: v,
          }),
        );
      }
    },
    { form, invalidate: financialKeys(business.id), successMessage: (_, v) => `Sent to ${v.to}`, onSuccess: onDone },
  );
  return (
    <form onSubmit={form.onSubmit((v) => send.mutate(v))} noValidate>
      <Stack>
        <TextInput label="To" type="email" data-autofocus {...form.getInputProps('to')} />
        <TextInput label="Subject" {...form.getInputProps('subject')} />
        <Textarea label="Message" autosize minRows={5} {...form.getInputProps('message')} />
        <Text size="xs" c="dimmed">
          The PDF is attached. Replies go to {business.email ?? 'your business email (add one in Settings)'}.
        </Text>
        <Group justify="flex-end">
          <Button variant="default" onClick={onDone}>
            Cancel
          </Button>
          <Button type="submit" loading={send.isPending}>
            Send {noun}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
