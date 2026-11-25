import { Button, Group, Stack, Switch, Text, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { formatDateTime } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';

const noticesEmailKey = ['me', 'notices-email'] as const;

interface Values {
  enabled: boolean;
  to: string;
}

function NoticesForm({
  enabled,
  to,
  defaultTo,
  lastSentAt,
}: {
  enabled: boolean;
  to: string;
  defaultTo: string | null;
  lastSentAt: string | null;
}) {
  const form = useForm<Values>({ initialValues: { enabled, to } });
  const save = useApiMutation(
    (v: Values) => unwrap(api.PUT('/api/v1/users/me/notices-email', { body: { enabled: v.enabled, to: v.to.trim() || null } })),
    { form, invalidate: [noticesEmailKey], successMessage: 'Notices email saved' },
  );
  const sendNow = useApiMutation(() => unwrap(api.POST('/api/v1/users/me/notices-email/run', { body: { now: true } })), {
    invalidate: [noticesEmailKey],
    successMessage: (r) => (r.sent ? `Sent: ${r.notices} ${r.notices === 1 ? 'notice' : 'notices'}` : (r.reason ?? 'Nothing to send')),
  });
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack gap="md">
        <Switch label="Email me my notices each week" {...form.getInputProps('enabled', { type: 'checkbox' })} />
        <TextInput
          label="Send to"
          type="email"
          placeholder={defaultTo ?? 'you@example.com'}
          description={defaultTo ? `Leave empty to use ${defaultTo}.` : 'Set up your email account above first.'}
          {...form.getInputProps('to')}
        />
        {lastSentAt && (
          <Text size="xs" c="dimmed">
            Last sent {formatDateTime(lastSentAt)}.
          </Text>
        )}
        <Group justify="flex-end">
          <Button variant="default" onClick={() => sendNow.mutate()} loading={sendNow.isPending}>
            Send now
          </Button>
          <Button type="submit" loading={save.isPending}>
            Save
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** Settings › Email › Weekly notices email (SL-606). */
export function NoticesEmailCard() {
  const settings = useQuery({ queryKey: noticesEmailKey, queryFn: () => unwrap(api.GET('/api/v1/users/me/notices-email')) });
  let content;
  if (settings.isPending) content = <ListSkeleton rows={2} />;
  else if (settings.isError) content = <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  else
    content = (
      <NoticesForm
        key={settings.data.last_sent_at ?? 'never'}
        enabled={settings.data.enabled}
        to={settings.data.to ?? ''}
        defaultTo={settings.data.default_to ?? null}
        lastSentAt={settings.data.last_sent_at ?? null}
      />
    );
  return (
    <SectionCard
      title="Weekly notices email"
      description="Once a week, when the app is open, an email listing the notices you haven’t dismissed (from Insights), for every business."
    >
      {content}
    </SectionCard>
  );
}

let ranThisSession = false;

/** Sends the weekly notices email when it's due, once each time the app opens. */
export function NoticesEmailRunner() {
  const queryClient = useQueryClient();
  const run = useMutation({
    mutationFn: () => unwrap(api.POST('/api/v1/users/me/notices-email/run', { body: { now: false } })),
    onSuccess: (r) => {
      if (r.sent) {
        void queryClient.invalidateQueries({ queryKey: noticesEmailKey });
        notifications.show({ title: 'Notices email', message: `Your weekly notices email was sent (${r.notices}).` });
      }
    },
  });
  const { mutate } = run;
  useEffect(() => {
    if (ranThisSession) return;
    ranThisSession = true;
    mutate();
  }, [mutate]);
  return null;
}
