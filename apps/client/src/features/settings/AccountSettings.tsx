import { Button, Group, Select, SegmentedControl, SimpleGrid, Stack, Text, TextInput } from '@mantine/core';
import { useForm } from '@mantine/form';
import { notifications } from '@mantine/notifications';
import type { Preferences } from '@solo/api-client';
import { zod4Resolver } from 'mantine-form-zod-resolver';
import { useState } from 'react';
import { z } from 'zod';
import { SectionCard } from '@/components/States';
import { accountKeys, useCurrentUser } from '@/features/account/api';
import { useSavePreferences } from '@/features/account/preferences';
import { api, fetchBlob, unwrap } from '@/services/api/client';
import { errorMessage } from '@/services/api/errors';
import { saveBlob } from '@/services/platform';
import { useApiMutation } from '@/services/api/query';

// ─── Profile ────────────────────────────────────────────────────────────────

const nameSchema = z.object({
  first_name: z.string().trim().max(100, 'Use up to 100 characters.'),
  last_name: z.string().trim().max(100, 'Use up to 100 characters.'),
});

export function ProfileSettings() {
  const user = useCurrentUser();
  const form = useForm({ initialValues: { first_name: user.first_name, last_name: user.last_name }, validate: zod4Resolver(nameSchema) });
  const save = useApiMutation(
    (values: { first_name: string; last_name: string }) =>
      unwrap(api.PUT('/api/v1/users/me', { body: { ...values, preferences: user.preferences } })),
    { form, successMessage: 'Profile saved', invalidate: [accountKeys.me] },
  );

  return (
    <Stack gap="lg">
      <SectionCard title="Your name" description="Shown in the activity log next to the changes you make.">
        <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
          <Stack>
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <TextInput label="First name" autoComplete="given-name" {...form.getInputProps('first_name')} />
              <TextInput label="Last name" autoComplete="family-name" {...form.getInputProps('last_name')} />
            </SimpleGrid>
            <Group justify="flex-end">
              <Button type="submit" loading={save.isPending} disabled={!form.isDirty()}>
                Save
              </Button>
            </Group>
          </Stack>
        </form>
      </SectionCard>
    </Stack>
  );
}

// ─── Preferences ────────────────────────────────────────────────────────────

export function PreferencesSettings() {
  const user = useCurrentUser();
  const save = useSavePreferences('Preferences saved');
  const update = (change: Partial<Preferences>) => save.mutate({ user, preferences: { ...user.preferences, ...change } });
  const prefs = user.preferences;
  return (
    <Stack gap="lg">
      <SectionCard title="Appearance">
        <Stack gap={6}>
          <Text size="sm" fw={500}>
            Theme
          </Text>
          <SegmentedControl
            value={prefs.theme}
            onChange={(theme) => update({ theme: theme as Preferences['theme'] })}
            data={[
              { value: 'light', label: 'Light' },
              { value: 'dark', label: 'Dark' },
              { value: 'auto', label: 'Match my device' },
            ]}
            maw={360}
          />
        </Stack>
      </SectionCard>
      <SectionCard title="Language and formats">
        <SimpleGrid cols={{ base: 1, sm: 3 }}>
          <Select
            label="Language"
            value={prefs.language}
            data={[{ value: 'en-NZ', label: 'English (New Zealand)' }]}
            onChange={(language) => language && update({ language })}
          />
          <Select
            label="Date format"
            value={prefs.date_format}
            data={[
              { value: 'DD/MM/YYYY', label: '31/12/2026' },
              { value: 'YYYY-MM-DD', label: '2026-12-31' },
              { value: 'MM/DD/YYYY', label: '12/31/2026' },
            ]}
            onChange={(date_format) => date_format && update({ date_format: date_format as Preferences['date_format'] })}
          />
          <Select
            label="Number format"
            value={prefs.number_locale}
            data={[
              { value: 'en-NZ', label: 'New Zealand (1,234.56)' },
              { value: 'en-AU', label: 'Australia (1,234.56)' },
              { value: 'en-GB', label: 'United Kingdom (1,234.56)' },
              { value: 'en-US', label: 'United States (1,234.56)' },
            ]}
            onChange={(number_locale) => number_locale && update({ number_locale })}
          />
        </SimpleGrid>
      </SectionCard>
    </Stack>
  );
}

// ─── Data ───────────────────────────────────────────────────────────────────

async function downloadExport() {
  const blob = await fetchBlob('/api/v1/users/me/export');
  saveBlob(blob, `solo-export-${new Date().toISOString().slice(0, 10)}.json`);
}

export function DataSettings() {
  const [exporting, setExporting] = useState(false);
  return (
    <Stack gap="lg">
      <SectionCard
        title="Download your data"
        description="A JSON file with your profile and every business, including settings and history."
      >
        <Button
          variant="default"
          loading={exporting}
          onClick={async () => {
            setExporting(true);
            try {
              await downloadExport();
            } catch (error) {
              notifications.show({ color: 'red', title: 'Download failed', message: errorMessage(error) });
            } finally {
              setExporting(false);
            }
          }}
        >
          Download data
        </Button>
      </SectionCard>
    </Stack>
  );
}
