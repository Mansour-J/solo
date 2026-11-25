import { Badge, Button, Checkbox, Group, Modal, Paper, Select, SimpleGrid, Stack, Text, TextInput } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { notifications } from '@mantine/notifications';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Repeat } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Link } from 'react-router';
import { Page } from '@/components/Page';
import { EmptyBlock, ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { financialKeys, useRecurringInvoices, type Frequency, type Invoice, type RecurringInvoice } from './api';
import { FREQUENCIES, frequencyLabel, nextMonth } from './labels';

interface ScheduleValues {
  name: string;
  frequency: Frequency;
  next_date: string | null;
  end_date: string | null;
  auto_issue: boolean;
}

function ScheduleFields({
  values,
  onChange,
  errors,
}: {
  values: ScheduleValues;
  onChange: (v: ScheduleValues) => void;
  errors: Record<string, string>;
}) {
  return (
    <Stack gap="sm">
      <TextInput
        label="Name"
        maxLength={120}
        value={values.name}
        onChange={(e) => onChange({ ...values, name: e.currentTarget.value })}
        error={errors.name}
      />
      <SimpleGrid cols={{ base: 1, sm: 3 }}>
        <Select
          label="How often"
          allowDeselect={false}
          data={FREQUENCIES.map((f) => ({ value: f.value, label: f.label }))}
          value={values.frequency}
          onChange={(v) => v && onChange({ ...values, frequency: v as Frequency })}
        />
        <DateInput
          label="Next invoice"
          valueFormat={dateInputFormat()}
          value={values.next_date}
          onChange={(v) => onChange({ ...values, next_date: v })}
          error={errors.next_date}
        />
        <DateInput
          label="Last invoice (optional)"
          valueFormat={dateInputFormat()}
          clearable
          value={values.end_date}
          onChange={(v) => onChange({ ...values, end_date: v })}
          error={errors.end_date}
        />
      </SimpleGrid>
      <Checkbox
        label="Issue each invoice straight away"
        description="Otherwise each one is a draft for you to check and issue."
        checked={values.auto_issue}
        onChange={(e) => onChange({ ...values, auto_issue: e.currentTarget.checked })}
      />
    </Stack>
  );
}

function fieldErrors(error: unknown): Record<string, string> {
  const fields = (error as { fields?: Record<string, string> } | null)?.fields;
  return fields ?? {};
}

/** “Repeat this invoice” on an invoice: makes a schedule from its customer, lines and terms. */
export function RepeatInvoiceModal({ invoice, opened, onClose }: { invoice: Invoice; opened: boolean; onClose: () => void }) {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const [values, setValues] = useState<ScheduleValues>({
    name: invoice.customer.name,
    frequency: 'monthly',
    next_date: nextMonth(invoice.issue_date),
    end_date: null,
    auto_issue: false,
  });
  const save = useApiMutation(
    () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/recurring-invoices', {
          params: { path: { business_id: business.id } },
          body: { invoice_id: invoice.id, ...values, next_date: values.next_date ?? '', end_date: values.end_date },
        }),
      ),
    { invalidate: financialKeys(business.id), successMessage: 'Recurring invoice set up', onSuccess: onClose },
  );
  return (
    <Modal opened={opened} onClose={onClose} title="Repeat this invoice" size="lg" fullScreen={isMobile}>
      <Stack gap="md">
        <Text size="sm">
          A new invoice to {invoice.customer.name} with the same lines is created on each date when you open the app. Change the lines by
          repeating a different invoice.
        </Text>
        <ScheduleFields values={values} onChange={setValues} errors={fieldErrors(save.error)} />
        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => save.mutate()} loading={save.isPending} disabled={!values.next_date || !values.name.trim()}>
            Repeat invoice
          </Button>
        </Group>
      </Stack>
    </Modal>
  );
}

function EditSchedule({ schedule, onClose }: { schedule: RecurringInvoice; onClose: () => void }) {
  const business = useCurrentBusiness();
  const [values, setValues] = useState<ScheduleValues>({
    name: schedule.name,
    frequency: schedule.frequency as Frequency,
    next_date: schedule.next_date,
    end_date: schedule.end_date ?? null,
    auto_issue: schedule.auto_issue,
  });
  const save = useApiMutation(
    () =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/recurring-invoices/{recurring_id}', {
          params: { path: { business_id: business.id, recurring_id: schedule.id } },
          body: { ...values, next_date: values.next_date ?? '', end_date: values.end_date },
        }),
      ),
    { invalidate: financialKeys(business.id), successMessage: 'Schedule saved', onSuccess: onClose },
  );
  return (
    <Stack gap="md">
      <ScheduleFields values={values} onChange={setValues} errors={fieldErrors(save.error)} />
      <Group justify="flex-end">
        <Button variant="default" onClick={onClose}>
          Cancel
        </Button>
        <Button onClick={() => save.mutate()} loading={save.isPending} disabled={!values.next_date}>
          Save changes
        </Button>
      </Group>
    </Stack>
  );
}

function ScheduleCard({ schedule, onEdit }: { schedule: RecurringInvoice; onEdit: () => void }) {
  const business = useCurrentBusiness();
  const toggle = useApiMutation(
    () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/recurring-invoices/{recurring_id}/archive', {
          params: { path: { business_id: business.id, recurring_id: schedule.id } },
          body: { archived: !schedule.archived },
        }),
      ),
    { invalidate: financialKeys(business.id), successMessage: schedule.archived ? 'Schedule restarted' : 'Schedule stopped' },
  );
  return (
    <Paper withBorder radius="md" p="md">
      <Group justify="space-between" align="flex-start" gap="sm">
        <Stack gap={2} style={{ minWidth: 0 }}>
          <Group gap={6}>
            <Text fw={600}>{schedule.name}</Text>
            {schedule.archived ? (
              <Badge color="gray" variant="outline">
                Stopped
              </Badge>
            ) : schedule.auto_issue ? (
              <Badge variant="light">Issued automatically</Badge>
            ) : (
              <Badge color="gray" variant="light">
                Drafts
              </Badge>
            )}
          </Group>
          <Text size="sm" c="dimmed">
            {schedule.customer_name} · {frequencyLabel(schedule.frequency)} · {schedule.lines.length}{' '}
            {schedule.lines.length === 1 ? 'line' : 'lines'}
          </Text>
          <Text size="sm">
            {schedule.archived ? 'Stopped' : `Next invoice ${formatDate(schedule.next_date)}`}
            {schedule.end_date ? `, last ${formatDate(schedule.end_date)}` : ''} · {schedule.invoices_created} created
          </Text>
        </Stack>
        <Group gap="xs">
          {!schedule.archived && (
            <Button size="xs" variant="default" onClick={onEdit}>
              Edit
            </Button>
          )}
          <Button
            size="xs"
            variant="subtle"
            color={schedule.archived ? undefined : 'red'}
            loading={toggle.isPending}
            onClick={() => toggle.mutate()}
          >
            {schedule.archived ? 'Restart' : 'Stop'}
          </Button>
        </Group>
      </Group>
    </Paper>
  );
}

/** Invoices › Recurring: schedules made from invoices with “Repeat this invoice”. */
export function RecurringPage() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const schedules = useRecurringInvoices(business.id);
  const [editing, setEditing] = useState<RecurringInvoice | null>(null);
  let content;
  if (schedules.isPending) content = <ListSkeleton rows={3} />;
  else if (schedules.isError) content = <ErrorState error={schedules.error} onRetry={() => void schedules.refetch()} />;
  else if (schedules.data.length === 0)
    content = (
      <EmptyBlock
        icon={Repeat}
        title="No recurring invoices"
        description="Open an invoice you send regularly and choose Repeat this invoice from More actions. A new one is then made on each date."
        action={
          <Button component={Link} to="/invoices">
            Open invoices
          </Button>
        }
      />
    );
  else
    content = (
      <Stack gap="sm">
        {schedules.data.map((s) => (
          <ScheduleCard key={s.id} schedule={s} onEdit={() => setEditing(s)} />
        ))}
      </Stack>
    );
  return (
    <Page
      title="Recurring invoices"
      description="Invoices made on a schedule from one you’ve sent before. Due ones are created when you open the app."
    >
      {content}
      <Modal opened={editing !== null} onClose={() => setEditing(null)} title="Edit schedule" size="lg" fullScreen={isMobile}>
        {editing && <EditSchedule key={editing.id} schedule={editing} onClose={() => setEditing(null)} />}
      </Modal>
    </Page>
  );
}

const ran = new Set<string>();

/** Creates the recurring invoices that have come due, once per business each time the app opens. */
export function RecurringRunner() {
  const business = useCurrentBusiness();
  const queryClient = useQueryClient();
  const run = useMutation({
    mutationFn: () =>
      unwrap(api.POST('/api/v1/businesses/{business_id}/recurring-invoices/run', { params: { path: { business_id: business.id } } })),
    onSuccess: (result) => {
      if ((result.created ?? 0) > 0) {
        void queryClient.invalidateQueries({ queryKey: ['business', business.id] });
        notifications.show({
          title: 'Recurring invoices',
          message:
            result.issued === result.created
              ? `${result.created} ${result.created === 1 ? 'invoice was' : 'invoices were'} created and issued.`
              : `${result.created} ${result.created === 1 ? 'invoice was' : 'invoices were'} created: check and issue the drafts.`,
        });
      }
      for (const problem of result.problems ?? []) notifications.show({ color: 'orange', title: 'Recurring invoice', message: problem });
    },
  });
  const { mutate } = run;
  useEffect(() => {
    if (ran.has(business.id)) return;
    ran.add(business.id);
    mutate();
  }, [business.id, mutate]);
  return null;
}
