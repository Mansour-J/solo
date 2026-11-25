import { Button, Divider, Group, Modal, NumberInput, SimpleGrid, Stack, Switch, Text, Textarea, TextInput, Title } from '@mantine/core';
import { useForm, type UseFormReturnType } from '@mantine/form';
import { Copy } from 'lucide-react';
import type { ReactNode } from 'react';
import { businessKeys, useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { customerKeys } from './api';
import {
  customerSchema,
  emptyCustomerForm,
  fromCustomer,
  toCustomerInput,
  type Customer,
  type CustomerDetail,
  type CustomerFormValues,
} from './customerForm';

type Form = UseFormReturnType<CustomerFormValues>;

function AddressFields({ form, prefix, label }: { form: Form; prefix: 'billing_address' | 'business_address'; label: string }) {
  return (
    <Stack gap="sm">
      <TextInput label="Street address" aria-label={`${label}: street address`} {...form.getInputProps(`${prefix}.line1`)} />
      <TextInput label="Address line 2" aria-label={`${label}: address line 2`} {...form.getInputProps(`${prefix}.line2`)} />
      <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
        <TextInput label="Suburb" aria-label={`${label}: suburb`} {...form.getInputProps(`${prefix}.suburb`)} />
        <TextInput label="Town or city" aria-label={`${label}: town or city`} {...form.getInputProps(`${prefix}.city`)} />
        <TextInput label="Region" aria-label={`${label}: region`} {...form.getInputProps(`${prefix}.region`)} />
        <TextInput label="Postcode" aria-label={`${label}: postcode`} {...form.getInputProps(`${prefix}.postcode`)} />
      </SimpleGrid>
    </Stack>
  );
}

function FormSection({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Stack gap="sm">
      <Divider />
      <Group justify="space-between" align="flex-start" gap="xs">
        <Stack gap={2}>
          <Title order={4}>{title}</Title>
          {description && (
            <Text size="xs" c="dimmed">
              {description}
            </Text>
          )}
        </Stack>
        {actions}
      </Group>
      {children}
    </Stack>
  );
}

function CustomerForm({
  customer,
  onDone,
  onCancel,
}: {
  customer: Customer | null;
  onDone: (saved: CustomerDetail) => void;
  onCancel: () => void;
}) {
  const business = useCurrentBusiness();
  const settings = useBusinessSettings(business.id);
  const form = useForm<CustomerFormValues>({
    initialValues: customer ? fromCustomer(customer) : emptyCustomerForm,
    validate: zodValidate<CustomerFormValues>(customerSchema),
  });
  const save = useApiMutation(
    (values: CustomerFormValues) => {
      const body = toCustomerInput(values, customer ?? undefined);
      return customer
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/customers/{customer_id}', {
              params: { path: { business_id: business.id, customer_id: customer.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/customers', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: (saved) => (customer ? `${saved.name} saved` : `${saved.name} added`),
      invalidate: [
        customerKeys.all(business.id),
        // Invoice and quote lists show customer names.
        ...(customer ? [businessKeys.invoices(business.id), businessKeys.quotes(business.id)] : []),
        businessKeys.dashboard(business.id),
        businessKeys.activityAll(business.id),
      ],
      onSuccess: (saved) => onDone(saved),
    },
  );
  const defaultTerms = settings.data?.default_payment_terms_days;

  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="lg">
        <Stack gap="sm">
          <TextInput
            label="Name"
            description="Shown in lists and on invoices: a person or an organisation."
            required
            data-autofocus
            {...form.getInputProps('name')}
          />
          <SimpleGrid cols={{ base: 1, sm: 2 }} spacing="sm">
            <TextInput label="Business name" description="If different from the name." {...form.getInputProps('business_name')} />
            <TextInput label="Contact person" {...form.getInputProps('contact_person')} />
            <TextInput label="Email" type="email" description="Invoices and quotes are sent here." {...form.getInputProps('email')} />
            <TextInput label="Phone" type="tel" {...form.getInputProps('phone')} />
          </SimpleGrid>
        </Stack>

        <FormSection title="Billing address" description="Printed on invoices and quotes.">
          <AddressFields form={form} prefix="billing_address" label="Billing address" />
        </FormSection>

        <FormSection
          title="Business address"
          description="Where the customer is based, if different."
          actions={
            <Button
              variant="subtle"
              size="compact-sm"
              leftSection={<Copy size={14} />}
              onClick={() => form.setFieldValue('business_address', { ...form.getValues().billing_address })}
            >
              Same as billing
            </Button>
          }
        >
          <AddressFields form={form} prefix="business_address" label="Business address" />
        </FormSection>

        <FormSection title="Tax and payment">
          <SimpleGrid cols={{ base: 1, sm: 3 }} spacing="sm">
            <TextInput label="NZBN" inputMode="numeric" description="13 digits." {...form.getInputProps('nzbn')} />
            <TextInput label="Tax number" description="GST or other tax number." {...form.getInputProps('tax_number')} />
            <NumberInput
              label="Payment terms (days)"
              description={
                defaultTerms === undefined ? 'Leave empty to use your default.' : `Leave empty to use your default (${defaultTerms} days).`
              }
              placeholder={defaultTerms === undefined ? undefined : String(defaultTerms)}
              min={0}
              max={365}
              allowDecimal={false}
              allowNegative={false}
              {...form.getInputProps('payment_terms_days')}
            />
          </SimpleGrid>
        </FormSection>

        <FormSection title="Notes" description="Private: not shown to the customer.">
          <Textarea aria-label="Notes" autosize minRows={2} maxRows={8} {...form.getInputProps('notes')} />
          {customer && (
            <Switch
              label="Active"
              description="Inactive customers are hidden from lists and can’t be used on new invoices or quotes."
              {...form.getInputProps('is_active', { type: 'checkbox' })}
            />
          )}
        </FormSection>

        <Group justify="flex-end">
          <Button variant="default" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {customer ? 'Save changes' : 'Add customer'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

/** Create (no `customer`) or edit a customer. The form starts fresh each time the modal opens. */
export function CustomerFormModal({
  opened,
  onClose,
  customer = null,
  onSaved,
}: {
  opened: boolean;
  onClose: () => void;
  customer?: Customer | null;
  onSaved?: (saved: CustomerDetail) => void;
}) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={customer ? `Edit ${customer.name}` : 'Add customer'} size="xl" fullScreen={isMobile}>
      <CustomerForm
        customer={customer}
        onCancel={onClose}
        onDone={(saved) => {
          onClose();
          onSaved?.(saved);
        }}
      />
    </Modal>
  );
}
