import {
  Badge,
  Button,
  ColorInput,
  Group,
  Modal,
  NumberInput,
  Pagination,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Textarea,
  TextInput,
} from '@mantine/core';
import { useForm } from '@mantine/form';
import { useDisclosure } from '@mantine/hooks';
import type { BusinessSettings as Settings } from '@solo/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { zodValidate } from '@/lib/forms';
import { Plus } from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from 'react-router';
import { z } from 'zod';
import { BRAND } from '@/branding/brand';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { businessKeys, useBusinessActivity, useBusinesses, useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { ContactFields, FinancialFields, IdentityFields, TaxFields } from '@/features/businesses/BusinessFields';
import { businessSchema, fromBusiness, toBusinessInput, type BusinessFormValues } from '@/features/businesses/businessForm';
import { LogoUploader } from '@/features/businesses/LogoUploader';
import { formatDateTime } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useSessionStore } from '@/stores/session';
import { ReminderSettingsCard } from '@/features/invoices/Reminders';

// ─── Business details ───────────────────────────────────────────────────────

export function BusinessDetailsSettings() {
  const business = useCurrentBusiness();
  const form = useForm<BusinessFormValues>({
    initialValues: fromBusiness(business),
    validate: zodValidate<BusinessFormValues>(businessSchema),
  });
  const save = useApiMutation(
    (values: BusinessFormValues) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}', {
          params: { path: { business_id: business.id } },
          body: toBusinessInput(values, true),
        }),
      ),
    {
      form,
      successMessage: 'Business details saved',
      invalidate: [businessKeys.all, businessKeys.detail(business.id), businessKeys.activityAll(business.id)],
      onSuccess: (updated) => form.resetDirty(fromBusiness(updated)),
    },
  );
  const submit = form.onSubmit((values) => save.mutate(values));
  const saveBar = (
    <Group justify="flex-end">
      <Button type="submit" loading={save.isPending} disabled={!form.isDirty()}>
        Save changes
      </Button>
    </Group>
  );

  return (
    <form onSubmit={submit} noValidate>
      <Stack gap="lg">
        <SectionCard title="Business">
          <IdentityFields form={form} />
        </SectionCard>
        <SectionCard title="Logo">
          <LogoUploader businessId={business.id} hasLogo={business.has_logo} />
        </SectionCard>
        <SectionCard title="Contact details" description="Printed on invoices and quotes.">
          <ContactFields form={form} />
        </SectionCard>
        <SectionCard title="Tax and GST">
          <TaxFields form={form} />
        </SectionCard>
        <SectionCard title="Financial year and reporting">
          <FinancialFields form={form} />
        </SectionCard>
        {saveBar}
      </Stack>
    </form>
  );
}

/** Keyed by business so switching business resets the form. */
export function KeyedBusinessDetails() {
  const business = useCurrentBusiness();
  return <BusinessDetailsSettings key={business.id} />;
}

// ─── Invoices & payments ────────────────────────────────────────────────────

const settingsSchema = z
  .object({
    invoice_prefix: z.string().max(20, 'Use 20 characters or fewer.'),
    quote_prefix: z.string().max(20, 'Use 20 characters or fewer.'),
    invoice_accent_color: z.string().refine((v) => v === '' || /^#[0-9a-fA-F]{6}$/.test(v), 'Use a colour like #13795b.'),
  })
  .passthrough();

function previewNumber(prefix: string, next: number, padding: number) {
  return `${prefix}${String(Math.max(1, next || 1)).padStart(Math.min(10, Math.max(1, padding || 1)), '0')}`;
}

function InvoicingForm({ settings, businessId }: { settings: Settings; businessId: string }) {
  const form = useForm<Settings>({ initialValues: settings, validate: zodValidate<Settings>(settingsSchema) });
  const save = useApiMutation(
    (values: Settings) =>
      unwrap(api.PUT('/api/v1/businesses/{business_id}/settings', { params: { path: { business_id: businessId } }, body: values })),
    {
      form,
      successMessage: 'Settings saved',
      invalidate: [businessKeys.settings(businessId), businessKeys.activityAll(businessId)],
      onSuccess: (updated) => form.resetDirty(updated),
    },
  );
  const v = form.values;
  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="lg">
        <SectionCard
          title="Numbering"
          description={`Next invoice: ${previewNumber(v.invoice_prefix, v.next_invoice_number, v.number_padding)} · next quote: ${previewNumber(v.quote_prefix, v.next_quote_number, v.number_padding)}`}
        >
          <SimpleGrid cols={{ base: 1, sm: 3 }}>
            <TextInput label="Invoice prefix" {...form.getInputProps('invoice_prefix')} />
            <NumberInput label="Next invoice number" min={1} allowDecimal={false} {...form.getInputProps('next_invoice_number')} />
            <NumberInput label="Digits" min={1} max={10} allowDecimal={false} {...form.getInputProps('number_padding')} />
            <TextInput label="Quote prefix" {...form.getInputProps('quote_prefix')} />
            <NumberInput label="Next quote number" min={1} allowDecimal={false} {...form.getInputProps('next_quote_number')} />
          </SimpleGrid>
        </SectionCard>
        <SectionCard title="Defaults">
          <Stack>
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <NumberInput
                label="Payment terms (days)"
                description="Due date = issue date + this many days."
                min={0}
                max={365}
                allowDecimal={false}
                {...form.getInputProps('default_payment_terms_days')}
              />
              <NumberInput
                label="Quotes valid for (days)"
                min={1}
                max={365}
                allowDecimal={false}
                {...form.getInputProps('default_quote_validity_days')}
              />
            </SimpleGrid>
            <Textarea
              label="Default invoice notes"
              description="Added to new invoices; you can change them per invoice."
              maxLength={2000}
              {...form.getInputProps('default_invoice_notes')}
            />
            <Textarea
              label="Footer"
              description="Small print at the bottom of every invoice."
              maxLength={2000}
              {...form.getInputProps('default_invoice_footer')}
            />
          </Stack>
        </SectionCard>
        <SectionCard title="Payment details" description="How customers pay you. Printed on every invoice.">
          <Stack>
            <SimpleGrid cols={{ base: 1, sm: 2 }}>
              <TextInput label="Account name" {...form.getInputProps('bank_account_name')} />
              <TextInput label="Account number" placeholder="12-3456-7890123-00" {...form.getInputProps('bank_account_number')} />
            </SimpleGrid>
            <Textarea
              label="Payment instructions"
              placeholder="Please use the invoice number as the reference."
              maxLength={2000}
              {...form.getInputProps('payment_instructions')}
            />
          </Stack>
        </SectionCard>
        <SectionCard title="Document branding">
          <ColorInput
            label="Accent colour"
            description={`Leave empty to use the ${BRAND.name} colour.`}
            format="hex"
            swatches={['#13795b', '#1d4ed8', '#7c3aed', '#be123c', '#c2410c', '#0f172a']}
            maw={280}
            {...form.getInputProps('invoice_accent_color')}
          />
        </SectionCard>
        <Group justify="flex-end">
          <Button type="submit" loading={save.isPending} disabled={!form.isDirty()}>
            Save changes
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

export function InvoicingSettings() {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  if (settings.isPending) return <ListSkeleton rows={6} height={64} />;
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  return (
    <Stack gap="lg">
      <InvoicingForm key={business.id} settings={settings.data} businessId={business.id} />
      <ReminderSettingsCard />
    </Stack>
  );
}

// ─── Activity log ───────────────────────────────────────────────────────────

export function ActivitySettings() {
  const business = useCurrentBusiness();
  const [page, setPage] = useState(1);
  const activity = useBusinessActivity(business.id, page);
  if (activity.isPending) return <ListSkeleton rows={8} height={40} />;
  if (activity.isError) return <ErrorState error={activity.error} onRetry={() => void activity.refetch()} />;
  const { items, total, per_page } = activity.data;
  return (
    <SectionCard
      title="Activity log"
      description="Every change to this business, with who made it and when. Entries can’t be edited or deleted."
    >
      {items.length === 0 ? (
        <EmptyBlock compact title="No activity yet" />
      ) : (
        <Stack>
          <Table.ScrollContainer minWidth={560}>
            <Table>
              <Table.Thead>
                <Table.Tr>
                  <Table.Th>Change</Table.Th>
                  <Table.Th>By</Table.Th>
                  <Table.Th>When</Table.Th>
                </Table.Tr>
              </Table.Thead>
              <Table.Tbody>
                {items.map((entry) => {
                  const fields = Object.keys((entry.changes as Record<string, unknown>) ?? {});
                  return (
                    <Table.Tr key={entry.id}>
                      <Table.Td>
                        <Text size="sm">{entry.summary}</Text>
                        {fields.length > 0 && (
                          <Group gap={4} mt={4}>
                            {fields.slice(0, 6).map((f) => (
                              <Badge key={f} variant="outline" color="gray" size="xs">
                                {f.replaceAll('_', ' ')}
                              </Badge>
                            ))}
                          </Group>
                        )}
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm">{entry.actor_name ?? 'Former user'}</Text>
                      </Table.Td>
                      <Table.Td>
                        <Text size="sm" c="dimmed" style={{ whiteSpace: 'nowrap' }}>
                          {formatDateTime(entry.created_at)}
                        </Text>
                      </Table.Td>
                    </Table.Tr>
                  );
                })}
              </Table.Tbody>
            </Table>
          </Table.ScrollContainer>
          {total > per_page && (
            <Group justify="flex-end">
              <Pagination total={Math.ceil(total / per_page)} value={page} onChange={setPage} size="sm" />
            </Group>
          )}
        </Stack>
      )}
    </SectionCard>
  );
}

// ─── Businesses ─────────────────────────────────────────────────────────────

function DeleteBusiness() {
  const business = useCurrentBusiness();
  const [opened, modal] = useDisclosure(false);
  const queryClient = useQueryClient();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const navigate = useNavigate();
  const form = useForm({ mode: 'uncontrolled', initialValues: { confirmation_name: '' } });
  const remove = useApiMutation(
    (values: { confirmation_name: string }) =>
      unwrap(api.DELETE('/api/v1/businesses/{business_id}', { params: { path: { business_id: business.id } }, body: values })),
    {
      form,
      successMessage: `${business.name} was deleted`,
      onSuccess: async () => {
        modal.close();
        selectBusiness(null);
        queryClient.removeQueries({ queryKey: businessKeys.detail(business.id) });
        await queryClient.invalidateQueries({ queryKey: businessKeys.all });
        navigate('/');
      },
    },
  );
  return (
    <SectionCard
      title={`Delete ${business.name}`}
      description="Permanently deletes this business and all of its records. This can’t be undone."
    >
      <Button color="red" variant="light" onClick={modal.open}>
        Delete business
      </Button>
      <Modal opened={opened} onClose={modal.close} title="Delete this business?">
        <form onSubmit={form.onSubmit((values) => remove.mutate(values))} noValidate>
          <Stack>
            <Text size="sm">
              Type <strong>{business.name}</strong> to confirm.
            </Text>
            <TextInput
              aria-label="Business name"
              autoComplete="off"
              key={form.key('confirmation_name')}
              {...form.getInputProps('confirmation_name')}
            />
            <Group justify="flex-end">
              <Button variant="default" onClick={modal.close}>
                Cancel
              </Button>
              <Button type="submit" color="red" loading={remove.isPending}>
                Delete permanently
              </Button>
            </Group>
          </Stack>
        </form>
      </Modal>
    </SectionCard>
  );
}

export function BusinessesSettings() {
  const current = useCurrentBusiness();
  const { data: businesses = [] } = useBusinesses();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const navigate = useNavigate();
  return (
    <Stack gap="lg">
      <SectionCard
        title="Your businesses"
        description="Each business keeps its own customers, records and settings."
        actions={
          <Button size="xs" leftSection={<Plus size={14} />} onClick={() => navigate('/onboarding')}>
            Add a business
          </Button>
        }
      >
        <Stack gap="xs">
          {businesses.map((b) => (
            <Group key={b.id} justify="space-between" py={6} style={{ borderBottom: '1px solid var(--app-border)' }}>
              <Group gap="sm">
                <Text fw={500}>{b.name}</Text>
                {b.id === current.id && <Badge color="brand">Current</Badge>}
              </Group>
              {b.id !== current.id && (
                <Button size="compact-xs" variant="subtle" onClick={() => selectBusiness(b.id)}>
                  Switch to
                </Button>
              )}
            </Group>
          ))}
        </Stack>
      </SectionCard>
      <DeleteBusiness />
    </Stack>
  );
}
