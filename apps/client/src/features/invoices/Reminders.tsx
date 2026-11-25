import { Alert, Button, Chip, Group, NumberInput, Stack, Switch, Text } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import type { components } from '@solo/api-client';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { Link } from 'react-router';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useEmailEnabled } from '@/features/server/api';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';

type Settings = components['schemas']['ReminderSettings'];

const settingsKey = (businessId: string) => ['business', businessId, 'reminder-settings'] as const;

/** The days after the due date people can choose from. */
const AFTER_CHOICES = [1, 3, 7, 14, 21, 30, 45, 60];

interface Values {
  reminders_enabled: boolean;
  reminder_days_before: number | string;
  reminder_days_after: string[];
}

function ReminderForm({ settings, businessId }: { settings: Settings; businessId: string }) {
  const emailEnabled = useEmailEnabled();
  const form = useForm<Values>({
    initialValues: {
      reminders_enabled: settings.reminders_enabled,
      reminder_days_before: settings.reminder_days_before ?? '',
      reminder_days_after: settings.reminder_days_after.map(String),
    },
  });
  const save = useApiMutation(
    (v: Values) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/reminders/settings', {
          params: { path: { business_id: businessId } },
          body: {
            reminders_enabled: v.reminders_enabled,
            reminder_days_before: v.reminder_days_before === '' ? null : Number(v.reminder_days_before),
            reminder_days_after: v.reminder_days_after.map(Number).sort((a, b) => a - b),
          },
        }),
      ),
    { form, invalidate: [settingsKey(businessId)], successMessage: 'Reminder settings saved' },
  );
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="md">
        {!emailEnabled && (
          <Alert color="orange" variant="light">
            Reminders are emailed, so they need your email account: set it up in{' '}
            <Text component={Link} to="/settings/email" inherit td="underline">
              Settings › Email
            </Text>
            .
          </Alert>
        )}
        <Switch label="Email reminders for unpaid invoices" {...form.getInputProps('reminders_enabled', { type: 'checkbox' })} />
        <NumberInput
          label="Days before the due date"
          description="A “coming up” reminder. Leave empty for none."
          min={1}
          max={60}
          allowDecimal={false}
          w={{ base: '100%', sm: 260 }}
          {...form.getInputProps('reminder_days_before')}
        />
        <Stack gap={6}>
          <Text size="sm" fw={500}>
            Days after the due date
          </Text>
          <Chip.Group multiple {...form.getInputProps('reminder_days_after')}>
            <Group gap="xs">
              {AFTER_CHOICES.map((d) => (
                <Chip key={d} value={String(d)} size="sm">
                  {d === 1 ? '1 day' : `${d} days`}
                </Chip>
              ))}
            </Group>
          </Chip.Group>
          {form.errors.reminder_days_after && (
            <Text size="xs" c="red">
              {form.errors.reminder_days_after}
            </Text>
          )}
        </Stack>
        <Text size="xs" c="dimmed">
          Reminders go when the app is open: each one once, with the invoice attached, and only while the invoice is unpaid. A reminder more
          than a week late is skipped. Turn them off for a customer on the customer’s page.
        </Text>
        <Group justify="flex-end">
          <Button type="submit" loading={save.isPending}>
            Save reminders
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** Settings › Invoices & payments › Reminders (SL-702). */
export function ReminderSettingsCard() {
  const business = useCurrentBusiness();
  const settings = useQuery({
    queryKey: settingsKey(business.id),
    queryFn: () =>
      unwrap(api.GET('/api/v1/businesses/{business_id}/reminders/settings', { params: { path: { business_id: business.id } } })),
  });
  let content;
  if (settings.isPending) content = <ListSkeleton rows={3} />;
  else if (settings.isError) content = <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  else content = <ReminderForm key={business.id} settings={settings.data} businessId={business.id} />;
  return (
    <SectionCard title="Reminders" description="Emails your customers before an invoice is due and after, until it’s paid.">
      {content}
    </SectionCard>
  );
}

/** The customer's own reminder switch. */
export function CustomerRemindersSwitch({ customerId, sendReminders }: { customerId: string; sendReminders: boolean }) {
  const business = useCurrentBusiness();
  const set = useApiMutation(
    (on: boolean) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/customers/{customer_id}/reminders', {
          params: { path: { business_id: business.id, customer_id: customerId } },
          body: { send_reminders: on },
        }),
      ),
    {
      invalidate: [['business', business.id, 'customers']],
      successMessage: (_, on) => (on ? 'Reminders on for this customer' : 'Reminders off for this customer'),
    },
  );
  return (
    <Switch
      label="Send invoice reminders"
      description="When reminders are on in Settings › Invoices & payments."
      checked={set.isPending ? set.variables : sendReminders}
      disabled={set.isPending}
      onChange={(e) => set.mutate(e.currentTarget.checked)}
    />
  );
}

const ran = new Set<string>();

/** Sends the reminders due today, once per business each time the app opens. */
export function ReminderRunner() {
  const business = useCurrentBusiness();
  const queryClient = useQueryClient();
  const run = useMutation({
    mutationFn: () =>
      unwrap(api.POST('/api/v1/businesses/{business_id}/reminders/run', { params: { path: { business_id: business.id } } })),
    onSuccess: (result) => {
      if (result.sent.length > 0) {
        void queryClient.invalidateQueries({ queryKey: ['business', business.id] });
        notifications.show({
          title: 'Invoice reminders',
          message: `${result.sent.length} ${result.sent.length === 1 ? 'reminder was' : 'reminders were'} emailed (${result.sent.map((s) => s.number).join(', ')}).`,
        });
      }
      if (result.stopped) notifications.show({ color: 'orange', title: 'Invoice reminders', message: result.stopped });
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
