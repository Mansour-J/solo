import { Alert, Badge, Button, Group, NumberInput, PasswordInput, Select, SimpleGrid, Stack, Switch, Text, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { CircleAlert, CircleCheck, Info } from 'lucide-react';
import { useState } from 'react';
import { z } from 'zod';
import { confirmAction } from '@/components/confirmAction';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { serverKeys } from '@/features/server/api';
import { formatDateTime } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { NoticesEmailCard } from './NoticesEmail';
import { emailSettingsKeys, systemKeys, useEmailSettings, type EmailSettingsView, type SmtpSecurity } from './systemApi';

interface Values {
  enabled: boolean;
  host: string;
  port: number | string;
  security: SmtpSecurity;
  username: string;
  password: string;
  from_name: string;
  from_address: string;
}

const schema = z
  .object({
    host: z.string().trim().min(1, 'Enter the mail server, e.g. smtp.gmail.com.'),
    port: z.coerce.number({ error: 'Enter a port, usually 587 or 465.' }).int().min(1, 'Enter a port, usually 587 or 465.').max(65535),
    from_address: z.string().trim().email('Enter the email address to send from.'),
  })
  .loose();

const SECURITY = [
  { value: 'starttls', label: 'STARTTLS (port 587)' },
  { value: 'tls', label: 'TLS (port 465)' },
  { value: 'none', label: 'None (a mail server on this network only)' },
];

/** Common providers' settings, to fill in the server, port and security. */
const PROVIDERS = [
  { value: 'gmail', label: 'Gmail', host: 'smtp.gmail.com', port: 587, security: 'starttls' as const },
  { value: 'outlook', label: 'Outlook / Microsoft 365', host: 'smtp.office365.com', port: 587, security: 'starttls' as const },
  { value: 'icloud', label: 'iCloud Mail', host: 'smtp.mail.me.com', port: 587, security: 'starttls' as const },
  { value: 'xtra', label: 'Xtra / Yahoo', host: 'smtp.mail.yahoo.com', port: 465, security: 'tls' as const },
];

function Status({ settings }: { settings: EmailSettingsView }) {
  if (!settings.configured) return null;
  if (settings.test_error)
    return (
      <Alert color="red" variant="light" icon={<CircleAlert size={16} />} title="The last test failed">
        {settings.test_error}
      </Alert>
    );
  if (settings.tested_at)
    return (
      <Alert color="teal" variant="light" icon={<CircleCheck size={16} />}>
        Working: a test email went out {formatDateTime(settings.tested_at)}.
      </Alert>
    );
  return (
    <Alert color="gray" variant="light" icon={<Info size={16} />}>
      Saved but not tested yet: send a test email to make sure it works.
    </Alert>
  );
}

function EmailForm({ settings }: { settings: EmailSettingsView }) {
  const form = useForm<Values>({
    initialValues: {
      enabled: settings.configured ? settings.enabled : true,
      host: settings.host,
      port: settings.port,
      security: settings.security,
      username: settings.username,
      password: '',
      from_name: settings.from_name,
      from_address: settings.from_address,
    },
    validate: zodValidate<Values>(schema),
  });
  const [testTo, setTestTo] = useState('');
  const invalidate = [emailSettingsKeys.all, serverKeys.info, systemKeys.status];
  const save = useApiMutation(
    (v: Values) =>
      unwrap(
        api.PUT('/api/v1/users/me/email-settings', {
          body: {
            enabled: v.enabled,
            host: v.host.trim(),
            port: Number(v.port),
            security: v.security,
            username: v.username.trim(),
            password: v.password || null,
            from_name: v.from_name.trim(),
            from_address: v.from_address.trim(),
          },
        }),
      ),
    { form, invalidate, successMessage: 'Email settings saved' },
  );
  const test = useApiMutation(() => unwrap(api.POST('/api/v1/users/me/email-settings/test', { body: { to: testTo.trim() || null } })), {
    invalidate,
    successMessage: 'Test email sent: check the inbox',
  });
  const remove = useApiMutation(() => unwrap(api.DELETE('/api/v1/users/me/email-settings')), {
    invalidate,
    successMessage: 'Email settings removed',
  });
  const confirmRemove = async () => {
    if (
      await confirmAction({
        title: 'Remove your email settings?',
        message: 'Invoices and statements then go back to being downloaded and sent from your own email.',
        confirmLabel: 'Remove',
        danger: true,
      })
    )
      remove.mutate();
  };
  return (
    <Stack gap="lg">
      <SectionCard
        title="Your email account"
        description="The app sends invoices, quotes, statements and reminders through your own email account, so they come from you. Use the settings your email provider gives for sending (SMTP)."
      >
        <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
          <Stack gap="md">
            <Status settings={settings} />
            <Select
              label="Fill in for"
              placeholder="Choose your provider, or enter the settings below"
              data={PROVIDERS.map(({ value, label }) => ({ value, label }))}
              value={null}
              onChange={(v) => {
                const p = PROVIDERS.find((x) => x.value === v);
                if (p) form.setValues({ host: p.host, port: p.port, security: p.security });
              }}
            />
            <SimpleGrid cols={{ base: 1, sm: 3 }}>
              <TextInput label="Mail server (SMTP)" withAsterisk placeholder="smtp.example.com" {...form.getInputProps('host')} />
              <NumberInput label="Port" withAsterisk min={1} max={65535} {...form.getInputProps('port')} />
              <Select label="Security" allowDeselect={false} data={SECURITY} {...form.getInputProps('security')} />
            </SimpleGrid>
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <TextInput
                label="User name"
                description="Usually your email address."
                autoComplete="off"
                {...form.getInputProps('username')}
              />
              <PasswordInput
                label="Password"
                description={
                  settings.has_password
                    ? 'Saved. Leave empty to keep it. Many providers need an app password.'
                    : 'Many providers (Gmail, Outlook, iCloud) need an app password, not your normal one.'
                }
                autoComplete="new-password"
                {...form.getInputProps('password')}
              />
              <TextInput label="Send from (email address)" withAsterisk {...form.getInputProps('from_address')} />
              <TextInput
                label="Name shown"
                description="Emails show your business name when you send documents."
                {...form.getInputProps('from_name')}
              />
            </SimpleGrid>
            <Switch label="Send email from the app" {...form.getInputProps('enabled', { type: 'checkbox' })} />
            <Text size="xs" c="dimmed">
              Your password is kept in the app’s database on this computer and used only to send email.
            </Text>
            <Group justify="space-between">
              {settings.configured ? (
                <Button variant="subtle" color="red" onClick={() => void confirmRemove()} loading={remove.isPending}>
                  Remove settings
                </Button>
              ) : (
                <span />
              )}
              <Button type="submit" loading={save.isPending}>
                Save email settings
              </Button>
            </Group>
          </Stack>
        </form>
      </SectionCard>
      {settings.configured && (
        <SectionCard title="Send a test email" description="Checks the settings by sending a short email.">
          <Group align="flex-end" gap="sm">
            <TextInput
              label="Send to"
              placeholder={settings.from_address}
              value={testTo}
              onChange={(e) => setTestTo(e.currentTarget.value)}
              style={{ flex: '1 1 240px' }}
            />
            <Button variant="default" onClick={() => test.mutate()} loading={test.isPending}>
              Send test email
            </Button>
          </Group>
        </SectionCard>
      )}
    </Stack>
  );
}

/** Settings › Email: the owner's mail account for sending documents (SL-015). */
export function EmailSettingsPage() {
  const settings = useEmailSettings();
  if (settings.isPending) return <ListSkeleton rows={4} />;
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  if (settings.data.server_managed)
    return (
      <Stack gap="lg">
        <SectionCard title="Email">
          <Group gap="xs">
            <Badge color="teal" variant="light">
              Working
            </Badge>
            <Text size="sm">This server sends email itself: there’s nothing to set up here.</Text>
          </Group>
        </SectionCard>
        <NoticesEmailCard />
      </Stack>
    );
  return (
    <Stack gap="lg">
      <EmailForm key={settings.data.tested_at ?? 'new'} settings={settings.data} />
      <NoticesEmailCard />
    </Stack>
  );
}
