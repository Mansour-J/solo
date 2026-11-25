import {
  Button,
  Divider,
  Group,
  Modal,
  NumberInput,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Switch,
  Text,
  Textarea,
  TextInput,
} from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import type { Business } from '@solo/api-client';
import { MoneyInput } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { CustomerSelect } from '@/features/customers/CustomerSelect';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { allocationChangeKeys, useDestinations, type Destination, type Rule } from './api';
import { emptyRuleValues, fromRule, ruleSchema, toRuleInput, type CapPeriod, type RuleFormValues } from './ruleForm';

const SOURCE_OPTIONS = [
  { value: 'any', label: 'All money received' },
  { value: 'invoice_payment', label: 'Invoice payments only' },
  { value: 'income', label: 'Income without an invoice only' },
];

const INCOME_TYPE_OPTIONS = [
  { value: '', label: 'All income' },
  { value: 'sales', label: 'Sales only' },
  { value: 'other', label: 'Other income only' },
];

const PERIOD_OPTIONS = [
  { value: 'week', label: 'Each week (Monday to Sunday)' },
  { value: 'fortnight', label: 'Each fortnight' },
  { value: 'month', label: 'Each calendar month' },
  { value: 'custom', label: 'Every number of days' },
];

/** Add a rule (no `rule`) or edit one. `destinationId` preselects the destination of a new rule. */
export function RuleFormModal({
  opened,
  onClose,
  rule,
  destinationId,
}: {
  opened: boolean;
  onClose: () => void;
  rule?: Rule | null;
  destinationId?: string | null;
}) {
  const isMobile = useIsMobile();
  return (
    <Modal opened={opened} onClose={onClose} title={rule ? 'Edit rule' : 'Add rule'} size="lg" fullScreen={isMobile}>
      <RuleFormLoader
        key={rule?.id ?? `new-${destinationId ?? ''}`}
        rule={rule ?? null}
        destinationId={destinationId ?? null}
        onClose={onClose}
      />
    </Modal>
  );
}

function RuleFormLoader({ rule, destinationId, onClose }: { rule: Rule | null; destinationId: string | null; onClose: () => void }) {
  const business = useCurrentBusiness();
  const destinations = useDestinations();
  if (destinations.isPending) return <ListSkeleton rows={6} height={44} />;
  if (destinations.isError) return <ErrorState error={destinations.error} onRetry={() => void destinations.refetch()} />;
  return <RuleForm business={business} destinations={destinations.data} rule={rule} destinationId={destinationId} onClose={onClose} />;
}

function RuleForm({
  business,
  destinations,
  rule,
  destinationId,
  onClose,
}: {
  business: Business;
  destinations: Destination[];
  rule: Rule | null;
  destinationId: string | null;
  onClose: () => void;
}) {
  const form = useForm<RuleFormValues>({
    initialValues: rule ? fromRule(rule, business.today) : emptyRuleValues(destinationId, business.today),
    validate: zodValidate<RuleFormValues>(ruleSchema),
  });
  const save = useApiMutation(
    (values: RuleFormValues) => {
      const body = toRuleInput(values);
      return rule
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/allocations/rules/{rule_id}', {
              params: { path: { business_id: business.id, rule_id: rule.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/allocations/rules', { params: { path: { business_id: business.id } }, body }));
    },
    {
      form,
      successMessage: rule ? 'Rule saved' : 'Rule added',
      invalidate: allocationChangeKeys(business.id),
      onSuccess: onClose,
    },
  );

  const v = form.values;
  // Archived destinations can't be chosen, but a rule may keep the one it already has.
  const destinationOptions = destinations
    .filter((d) => !d.archived || d.id === rule?.destination_id)
    .map((d) => ({ value: d.id, label: d.archived ? `${d.name} (archived)` : d.name }));
  const destination = destinations.find((d) => d.id === v.destination_id);

  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack gap="md">
        <Select
          label="Destination"
          placeholder="Choose a destination"
          withAsterisk
          searchable
          data={destinationOptions}
          {...form.getInputProps('destination_id')}
        />
        <Stack gap={6}>
          <Text size="sm" fw={500}>
            Amount to set aside
          </Text>
          <SegmentedControl
            aria-label="Amount to set aside"
            value={v.method}
            onChange={(value) => form.setFieldValue('method', value as RuleFormValues['method'])}
            data={[
              { value: 'percentage', label: 'A percentage' },
              { value: 'fixed', label: 'A fixed amount' },
            ]}
            w={{ base: '100%', xs: 'auto' }}
          />
        </Stack>
        {v.method === 'percentage' ? (
          <SimpleGrid cols={{ base: 1, xs: 2 }}>
            <TextInput
              label="Percentage"
              placeholder="e.g. 20"
              inputMode="decimal"
              withAsterisk
              rightSection={
                <Text size="sm" c="dimmed">
                  %
                </Text>
              }
              {...form.getInputProps('percentage')}
            />
            <Select
              label="Of"
              allowDeselect={false}
              data={[
                { value: 'received', label: 'The amount received' },
                { value: 'remaining', label: 'What earlier rules leave' },
              ]}
              value={v.basis}
              onChange={(value) => form.setFieldValue('basis', (value ?? 'received') as RuleFormValues['basis'])}
            />
          </SimpleGrid>
        ) : (
          <MoneyInput
            label="Amount from each receipt"
            withAsterisk
            currency={business.currency}
            value={v.fixed_amount}
            onChange={(value) => form.setFieldValue('fixed_amount', value)}
            error={form.errors.fixed_amount}
            w={{ base: '100%', xs: 240 }}
          />
        )}

        <Divider label="Applies to" labelPosition="left" />
        <SimpleGrid cols={{ base: 1, xs: 2 }}>
          <Select
            label="Money received"
            allowDeselect={false}
            data={SOURCE_OPTIONS}
            value={v.source}
            onChange={(value) => {
              form.setFieldValue('source', (value ?? 'any') as RuleFormValues['source']);
              if (value !== 'income') form.setFieldValue('income_type', '');
            }}
          />
          {v.source === 'income' && (
            <Select
              label="Income type"
              allowDeselect={false}
              data={INCOME_TYPE_OPTIONS}
              value={v.income_type}
              onChange={(value) => form.setFieldValue('income_type', (value ?? '') as RuleFormValues['income_type'])}
              error={form.errors.income_type}
            />
          )}
        </SimpleGrid>
        <CustomerSelect
          label="Customer"
          description="Optional. Only money from this customer."
          clearable
          value={v.customer_id}
          onChange={(id) => form.setFieldValue('customer_id', id)}
          error={form.errors.customer_id}
        />
        <SimpleGrid cols={{ base: 1, xs: 2 }}>
          <DateInput
            label="Received from"
            description="Optional."
            clearable
            valueFormat={dateInputFormat()}
            {...form.getInputProps('active_from')}
          />
          <DateInput
            label="Received until"
            description="Optional."
            clearable
            valueFormat={dateInputFormat()}
            {...form.getInputProps('active_to')}
          />
        </SimpleGrid>

        <Divider label="Limits" labelPosition="left" />
        <MoneyInput
          label="Most from one receipt"
          description="Optional."
          currency={business.currency}
          value={v.max_amount}
          onChange={(value) => form.setFieldValue('max_amount', value)}
          error={form.errors.max_amount}
          w={{ base: '100%', xs: 240 }}
        />
        <Switch label="Cap the total for each period" {...form.getInputProps('capped', { type: 'checkbox' })} />
        {v.capped && (
          <SimpleGrid cols={{ base: 1, xs: 2 }}>
            <MoneyInput
              label="Cap"
              withAsterisk
              currency={business.currency}
              value={v.cap_amount}
              onChange={(value) => form.setFieldValue('cap_amount', value)}
              error={form.errors.cap_amount}
            />
            <Select
              label="Period"
              allowDeselect={false}
              data={PERIOD_OPTIONS}
              value={v.cap_period}
              onChange={(value) => form.setFieldValue('cap_period', (value ?? 'month') as CapPeriod)}
              error={form.errors.cap_period}
            />
            {v.cap_period === 'custom' && (
              <NumberInput
                label="Days in each period"
                min={1}
                max={366}
                allowDecimal={false}
                withAsterisk
                {...form.getInputProps('cap_period_days')}
              />
            )}
            {(v.cap_period === 'fortnight' || v.cap_period === 'custom') && (
              <DateInput label="First period starts" withAsterisk valueFormat={dateInputFormat()} {...form.getInputProps('cap_anchor')} />
            )}
          </SimpleGrid>
        )}
        <Switch
          label="Stop when the destination reaches its target"
          description={
            destination && !destination.target_amount ? 'This destination has no target yet. Set one in Destinations.' : undefined
          }
          {...form.getInputProps('stop_at_target', { type: 'checkbox' })}
        />
        <Textarea label="Notes" autosize minRows={2} maxLength={1000} {...form.getInputProps('notes')} />
        <Group justify="flex-end" gap="xs">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {rule ? 'Save' : 'Add rule'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
