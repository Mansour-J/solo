import { Badge, Button, Group, Modal, SimpleGrid, Stack, Text, Textarea, TextInput, Tooltip } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { z } from 'zod';
import { SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { dateInputFormat, formatDateTime } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';

type EmailLog = components['schemas']['EmailLogRow'];

const statementEmailKeys = (businessId: string, customerId: string) =>
  ['businesses', businessId, 'customers', customerId, 'statement-emails'] as const;

interface Values {
  from: string | null;
  to: string | null;
  email: string;
  subject: string;
  message: string;
}

function StatementForm({ customer, onDone }: { customer: { id: string; name: string; email?: string | null }; onDone: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<Values>({
    initialValues: {
      from: business.current_financial_year.start,
      to: business.today,
      email: customer.email ?? '',
      subject: `Statement from ${business.name}`,
      message: `Hi ${customer.name},\n\nPlease find your statement attached.\n\nThanks,\n${business.name}`,
    },
    validate: zodValidate<Values>(
      z
        .object({
          email: z.string().trim().email('Enter the customer’s email address.'),
          from: z.string({ error: 'Choose the start of the period.' }).min(1, 'Choose the start of the period.'),
          to: z.string({ error: 'Choose the end of the period.' }).min(1, 'Choose the end of the period.'),
        })
        .loose(),
    ),
  });
  const send = useApiMutation(
    (v: Values) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/customers/{customer_id}/statement-emails', {
          params: { path: { business_id: business.id, customer_id: customer.id } },
          body: { from: v.from, to: v.to, email: v.email.trim(), subject: v.subject, message: v.message },
        }),
      ),
    {
      form,
      invalidate: [statementEmailKeys(business.id, customer.id)],
      successMessage: (_, v) => `Statement sent to ${v.email.trim()}`,
      onSuccess: onDone,
    },
  );
  return (
    <form onSubmit={form.onSubmit((v) => send.mutate(v))} noValidate>
      <Stack>
        <SimpleGrid cols={2}>
          <DateInput label="From" valueFormat={dateInputFormat()} {...form.getInputProps('from')} />
          <DateInput label="To" valueFormat={dateInputFormat()} {...form.getInputProps('to')} />
        </SimpleGrid>
        <TextInput label="Email to" type="email" data-autofocus {...form.getInputProps('email')} />
        <TextInput label="Subject" {...form.getInputProps('subject')} />
        <Textarea label="Message" autosize minRows={5} {...form.getInputProps('message')} />
        <Text size="xs" c="dimmed">
          The statement PDF for the period is attached, with the amount owing. Replies go to{' '}
          {business.email ?? 'your business email (add one in Settings)'}.
        </Text>
        <Group justify="flex-end">
          <Button variant="default" onClick={onDone}>
            Cancel
          </Button>
          <Button type="submit" loading={send.isPending}>
            Send statement
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** Emails the customer their statement of account for a period (SL-513). */
export function EmailStatementModal({
  opened,
  onClose,
  customer,
}: {
  opened: boolean;
  onClose: () => void;
  customer: { id: string; name: string; email?: string | null };
}) {
  return (
    <Modal opened={opened} onClose={onClose} title="Email statement" size="lg">
      {opened && <StatementForm customer={customer} onDone={onClose} />}
    </Modal>
  );
}

/** The statements emailed to this customer; nothing when there are none. */
export function StatementEmailsCard({ customerId }: { customerId: string }) {
  const business = useCurrentBusiness();
  const emails = useQuery({
    queryKey: statementEmailKeys(business.id, customerId),
    queryFn: () =>
      unwrap(
        api.GET('/api/v1/businesses/{business_id}/customers/{customer_id}/statement-emails', {
          params: { path: { business_id: business.id, customer_id: customerId } },
        }),
      ),
  });
  const rows: EmailLog[] = emails.data ?? [];
  if (rows.length === 0) return null;
  return (
    <SectionCard title="Statements emailed">
      <Stack gap={6}>
        {rows.map((e) => (
          <Group key={e.id} justify="space-between" wrap="nowrap">
            <div style={{ minWidth: 0 }}>
              <Text size="sm" className="clamp-1">
                {e.to_email}
              </Text>
              <Text size="xs" c="dimmed">
                {formatDateTime(e.created_at)}
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
