import { ActionIcon, Badge, Button, Checkbox, Group, Modal, NumberInput, Stack, Text, TextInput, Tooltip } from '@mantine/core';
import { useForm } from '@mantine/form';
import type { Business, BusinessSettings } from '@solo/api-client';
import { Archive, ArchiveRestore, Pencil, Plus } from 'lucide-react';
import { useState } from 'react';
import { z } from 'zod';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { businessKeys, useBusinessSettings, useCurrentBusiness } from '@/features/businesses/api';
import { formatNumber } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { expenseChangeKeys, expenseKeys, useExpenseCategories, type ExpenseCategory } from './api';
import { hundredthsToString, percentHundredths } from './expenseMath';

// ─── Categories ─────────────────────────────────────────────────────────────

interface NameValues {
  name: string;
  non_deductible: boolean;
  cost_of_sales: boolean;
}

const nameSchema = z.object({
  name: z.string().trim().min(1, 'Enter a name.').max(80, 'Use 80 characters or fewer.'),
  non_deductible: z.boolean(),
  cost_of_sales: z.boolean(),
});

function CategoryNameForm({
  businessId,
  category,
  onClose,
}: {
  businessId: string;
  category: ExpenseCategory | null;
  onClose: () => void;
}) {
  const form = useForm<NameValues>({
    initialValues: {
      name: category?.name ?? '',
      non_deductible: category?.non_deductible ?? false,
      cost_of_sales: category?.cost_of_sales ?? false,
    },
    validate: zodValidate<NameValues>(nameSchema),
  });
  const save = useApiMutation(
    (values: NameValues) => {
      const body = { name: values.name.trim(), non_deductible: values.non_deductible, cost_of_sales: values.cost_of_sales };
      return category
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/expense-categories/{category_id}', {
              params: { path: { business_id: businessId, category_id: category.id } },
              body,
            }),
          )
        : unwrap(api.POST('/api/v1/businesses/{business_id}/expense-categories', { params: { path: { business_id: businessId } }, body }));
    },
    {
      form,
      successMessage: (row) => (category ? `${row.name} saved` : `${row.name} added`),
      invalidate: expenseChangeKeys(businessId),
      onSuccess: onClose,
    },
  );
  return (
    <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
      <Stack>
        <TextInput label="Name" data-autofocus maxLength={80} {...form.getInputProps('name')} />
        <Checkbox
          label="Not deductible for tax"
          description="For costs you can’t claim, such as IRD penalties. Reports show them as an adjustment below net profit."
          {...form.getInputProps('non_deductible', { type: 'checkbox' })}
        />
        <Checkbox
          label="Cost of sales"
          description="For direct costs of what you sell, such as materials or stock. Reports show them before gross profit."
          {...form.getInputProps('cost_of_sales', { type: 'checkbox' })}
        />
        <Group justify="flex-end">
          <Button variant="default" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {category ? 'Save' : 'Add category'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

function CategoryRow({
  category,
  busy,
  onRename,
  onArchive,
}: {
  category: ExpenseCategory;
  busy: boolean;
  onRename: () => void;
  onArchive: (archived: boolean) => void;
}) {
  const count = category.expense_count;
  return (
    <Group justify="space-between" wrap="nowrap" gap="sm" py={8} style={{ borderBottom: '1px solid var(--app-border)' }}>
      <Stack gap={0} style={{ minWidth: 0 }}>
        <Group gap={6} wrap="nowrap">
          <Text size="sm" fw={500} truncate="end" c={category.archived ? 'dimmed' : undefined}>
            {category.name}
          </Text>
          {category.non_deductible && (
            <Badge color="gray" variant="light">
              Not deductible
            </Badge>
          )}
          {category.cost_of_sales && (
            <Badge color="gray" variant="light">
              Cost of sales
            </Badge>
          )}
          {category.archived && (
            <Badge color="gray" variant="outline">
              Archived
            </Badge>
          )}
        </Group>
        <Text size="xs" c="dimmed">
          {count === 0 ? 'No expenses' : `${formatNumber(count)} expense${count === 1 ? '' : 's'}`} · account {category.account_code}
        </Text>
      </Stack>
      {
        <Group gap={2} wrap="nowrap">
          <Tooltip label="Edit">
            <ActionIcon aria-label={`Edit ${category.name}`} onClick={onRename}>
              <Pencil size={16} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label={category.archived ? 'Restore' : 'Archive'}>
            <ActionIcon
              aria-label={`${category.archived ? 'Restore' : 'Archive'} ${category.name}`}
              loading={busy}
              onClick={() => onArchive(!category.archived)}
            >
              {category.archived ? <ArchiveRestore size={16} /> : <Archive size={16} />}
            </ActionIcon>
          </Tooltip>
        </Group>
      }
    </Group>
  );
}

function CategoriesSection({ business }: { business: Business }) {
  const categories = useExpenseCategories();
  const [dialog, setDialog] = useState<{ opened: boolean; category: ExpenseCategory | null }>({ opened: false, category: null });
  const [showArchived, setShowArchived] = useState(false);
  const archive = useApiMutation(
    ({ category, archived }: { category: ExpenseCategory; archived: boolean }) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/expense-categories/{category_id}/archive', {
          params: { path: { business_id: business.id, category_id: category.id } },
          body: { archived },
        }),
      ),
    {
      successMessage: (row, { archived }) => (archived ? `${row.name} archived` : `${row.name} restored`),
      invalidate: expenseChangeKeys(business.id),
    },
  );
  const close = () => setDialog((d) => ({ ...d, opened: false }));

  let body;
  if (categories.isPending) body = <ListSkeleton rows={6} height={40} />;
  else if (categories.isError) body = <ErrorState error={categories.error} onRetry={() => void categories.refetch()} />;
  else {
    const active = categories.data.filter((c) => !c.archived);
    const archived = categories.data.filter((c) => c.archived);
    const shown = showArchived ? [...active, ...archived] : active;
    body = (
      <Stack gap={0}>
        {active.length === 0 && !showArchived && (
          <EmptyBlock compact title="No active categories" description="Add a category to start recording expenses." />
        )}
        {shown.map((category) => (
          <CategoryRow
            key={category.id}
            category={category}
            busy={archive.isPending && archive.variables?.category.id === category.id}
            onRename={() => setDialog({ opened: true, category })}
            onArchive={(next) => archive.mutate({ category, archived: next })}
          />
        ))}
        {archived.length > 0 && (
          <Group mt="sm">
            <Button variant="subtle" size="compact-sm" onClick={() => setShowArchived((v) => !v)}>
              {showArchived ? 'Hide archived categories' : `Show archived categories (${archived.length})`}
            </Button>
          </Group>
        )}
      </Stack>
    );
  }

  return (
    <SectionCard
      title="Expense categories"
      description="Group your expenses for reports and your tax return. Archived categories keep their expenses but can’t be chosen for new ones."
      actions={
        <Button size="xs" leftSection={<Plus size={14} />} onClick={() => setDialog({ opened: true, category: null })}>
          Add category
        </Button>
      }
    >
      {body}
      <Modal opened={dialog.opened} onClose={close} title={dialog.category ? 'Edit category' : 'Add category'} size="sm">
        <CategoryNameForm key={dialog.category?.id ?? 'new'} businessId={business.id} category={dialog.category} onClose={close} />
      </Modal>
    </SectionCard>
  );
}

// ─── Default deductible share ───────────────────────────────────────────────

interface DeductibleValues {
  default_deductible_percentage: number | string;
}

const deductibleSchema = z.object({
  default_deductible_percentage: z
    .union([z.number(), z.string()])
    .refine((v) => percentHundredths(v) !== null, 'Enter a percentage between 0 and 100, with up to 2 decimal places.'),
});

function toFormValue(value: string): number | string {
  const hundredths = percentHundredths(value);
  return hundredths === null ? '' : hundredths / 100;
}

function DefaultDeductibleForm({ business, settings }: { business: Business; settings: BusinessSettings }) {
  const form = useForm<DeductibleValues>({
    initialValues: { default_deductible_percentage: toFormValue(settings.default_deductible_percentage) },
    validate: zodValidate<DeductibleValues>(deductibleSchema),
  });
  const save = useApiMutation(
    (values: DeductibleValues) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/settings', {
          params: { path: { business_id: business.id } },
          body: {
            ...settings,
            default_deductible_percentage: hundredthsToString(percentHundredths(values.default_deductible_percentage) ?? 10_000),
          },
        }),
      ),
    {
      form,
      successMessage: 'Default deductible share saved',
      invalidate: [
        businessKeys.settings(business.id),
        expenseKeys.all(business.id),
        businessKeys.dashboard(business.id),
        businessKeys.activityAll(business.id),
      ],
      onSuccess: (updated) => form.resetDirty({ default_deductible_percentage: toFormValue(updated.default_deductible_percentage) }),
    },
  );
  return (
    <SectionCard
      title="Default deductible share"
      description="The share of a business expense you claim as a tax deduction when you don’t enter one. It never exceeds the business share of a partly business expense."
    >
      <form onSubmit={form.onSubmit((values) => save.mutate(values))} noValidate>
        <Stack gap="sm">
          <Group align="flex-start" gap="sm">
            <NumberInput
              aria-label="Default deductible share"
              suffix="%"
              min={0}
              max={100}
              decimalScale={2}
              w={160}
              {...form.getInputProps('default_deductible_percentage')}
            />
            <Button type="submit" loading={save.isPending} disabled={!form.isDirty()}>
              Save
            </Button>
          </Group>
        </Stack>
      </form>
    </SectionCard>
  );
}

function DefaultDeductibleSection({ business }: { business: Business }) {
  const settings = useBusinessSettings(business.id);
  if (settings.isPending) return <ListSkeleton rows={1} height={120} />;
  if (settings.isError) return <ErrorState error={settings.error} onRetry={() => void settings.refetch()} />;
  return <DefaultDeductibleForm business={business} settings={settings.data} />;
}

/** Settings section: expense categories and the default deductible share. */
export function ExpenseCategoriesSettings() {
  const business = useCurrentBusiness();
  return (
    <Stack gap="lg" key={business.id}>
      <CategoriesSection business={business} />
      <DefaultDeductibleSection business={business} />
    </Stack>
  );
}
