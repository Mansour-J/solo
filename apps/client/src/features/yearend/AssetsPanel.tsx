import {
  ActionIcon,
  Badge,
  Button,
  Checkbox,
  Group,
  Modal,
  Paper,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Table,
  Text,
  Textarea,
  TextInput,
  Tooltip,
} from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useForm } from '@mantine/form';
import { Archive, ArchiveRestore, Laptop, Plus } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { MoneyInput, MoneyText } from '@/components/Money';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { useIsMobile } from '@/hooks/useIsMobile';
import { dateInputFormat, formatDate } from '@/lib/format';
import { zodValidate } from '@/lib/forms';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useAssets, useDepreciationYear, yearEndChangeKeys, type Asset } from './api';
import { AssetDisposal } from './AssetDisposal';
import { ChangeOfUse } from './ChangeOfUse';
import { assetBody, assetSchema, CHANGE_OF_USE_FROM, recentYearEnds, type AssetValues } from './forms';

function AssetForm({ asset, onDone }: { asset: Asset | null; onDone: () => void }) {
  const business = useCurrentBusiness();
  const form = useForm<AssetValues>({
    initialValues: asset
      ? {
          name: asset.name,
          asset_group: asset.asset_group,
          cost: asset.cost,
          purchase_gst: asset.purchase_gst,
          acquired_on: asset.acquired_on,
          method: asset.method === 'sl' || asset.method === 'full' ? asset.method : 'dv',
          rate: String(Number(asset.rate)),
          private_use: String(Number(asset.private_use)),
          brought_forward: asset.opening_value !== null && asset.opening_value !== undefined,
          opening_value: asset.opening_value ?? null,
          notes: asset.notes,
        }
      : {
          name: '',
          asset_group: 'Computer equipment',
          cost: null,
          purchase_gst: null,
          acquired_on: business.today,
          method: 'dv',
          rate: '',
          private_use: '0',
          brought_forward: false,
          opening_value: null,
          notes: '',
        },
    validate: zodValidate<AssetValues>(assetSchema),
  });
  // IRD's low-value asset rule: $1,000 or less excluding GST, bought after the opening balances.
  const lowValue = (form.values.cost ?? 0) > 0 && (form.values.cost ?? 0) <= 100_000 && !form.values.brought_forward;
  const save = useApiMutation(
    (v: AssetValues) =>
      asset
        ? unwrap(
            api.PUT('/api/v1/businesses/{business_id}/assets/{asset_id}', {
              params: { path: { business_id: business.id, asset_id: asset.id } },
              body: assetBody(v),
            }),
          )
        : unwrap(
            api.POST('/api/v1/businesses/{business_id}/assets', { params: { path: { business_id: business.id } }, body: assetBody(v) }),
          ),
    {
      form,
      invalidate: yearEndChangeKeys(business.id),
      successMessage: (row) => `${row.name} ${asset ? 'saved' : 'added'}`,
      onSuccess: onDone,
    },
  );
  const broughtForward = form.values.brought_forward;
  return (
    <form onSubmit={form.onSubmit((v) => save.mutate(v))} noValidate>
      <Stack>
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          <TextInput label="Name" placeholder="Laptop" data-autofocus maxLength={120} {...form.getInputProps('name')} />
          <TextInput
            label="Group"
            description="Assets are grouped on the schedule."
            maxLength={80}
            {...form.getInputProps('asset_group')}
          />
          <DateInput label="Date bought" valueFormat={dateInputFormat()} {...form.getInputProps('acquired_on')} />
          <MoneyInput
            label="Cost excluding GST"
            currency={business.currency}
            value={form.values.cost}
            onChange={(v) => form.setFieldValue('cost', v)}
            error={form.errors.cost}
          />
        </SimpleGrid>
        <Checkbox
          label="I had this before my opening balances"
          description="For an asset already on your accountant’s depreciation schedule: enter its book value at the opening balance date."
          {...form.getInputProps('brought_forward', { type: 'checkbox' })}
        />
        <SimpleGrid cols={{ base: 1, sm: 2 }}>
          {broughtForward ? (
            <MoneyInput
              label="Value at the opening balance date"
              currency={business.currency}
              value={form.values.opening_value}
              onChange={(v) => form.setFieldValue('opening_value', v)}
              error={form.errors.opening_value}
            />
          ) : (
            <MoneyInput
              label="GST paid"
              description="Claimed in the period you bought it."
              currency={business.currency}
              value={form.values.purchase_gst}
              onChange={(v) => form.setFieldValue('purchase_gst', v)}
              error={form.errors.purchase_gst}
            />
          )}
          <Stack gap={4}>
            <Text size="sm" fw={500}>
              Method
            </Text>
            <SegmentedControl
              data={[
                { value: 'dv', label: 'Diminishing value' },
                { value: 'sl', label: 'Straight line' },
                ...(lowValue || form.values.method === 'full' ? [{ value: 'full', label: 'Deduct in full' }] : []),
              ]}
              value={form.values.method}
              onChange={(v) => {
                form.setFieldValue('method', v as AssetValues['method']);
                if (v === 'full') form.setFieldValue('rate', '100');
              }}
            />
            {form.errors.method && (
              <Text size="xs" c="red">
                {form.errors.method}
              </Text>
            )}
            {lowValue && (
              <Text size="xs" c="dimmed">
                IRD lets an asset costing $1,000 or less (excluding GST) be deducted in full in the year you bought it.
              </Text>
            )}
          </Stack>
          {form.values.method !== 'full' && (
            <TextInput
              label="Rate (% a year)"
              description="From IRD’s rate finder or your accountant."
              inputMode="decimal"
              {...form.getInputProps('rate')}
            />
          )}
          <TextInput label="Private use (%)" inputMode="decimal" {...form.getInputProps('private_use')} />
        </SimpleGrid>
        <Textarea label="Notes" autosize minRows={2} maxLength={1000} {...form.getInputProps('notes')} />
        <Group justify="flex-end">
          <Button variant="default" onClick={onDone}>
            Cancel
          </Button>
          <Button type="submit" loading={save.isPending}>
            {asset ? 'Save' : 'Add asset'}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}

function rate(asset: Asset) {
  if (asset.method === 'full') return 'Deducted in full';
  return `${Number(asset.rate)}% ${asset.method === 'sl' ? 'SL' : 'DV'}`;
}

function AssetList({ assets, onOpen, onArchive }: { assets: Asset[]; onOpen?: (a: Asset) => void; onArchive?: (a: Asset) => void }) {
  const isMobile = useIsMobile();
  const archiveButton = (a: Asset) =>
    onArchive && (
      <Tooltip label={a.archived ? 'Restore' : 'Archive'}>
        <ActionIcon
          aria-label={`${a.archived ? 'Restore' : 'Archive'} ${a.name}`}
          onClick={(event) => {
            event.stopPropagation();
            onArchive(a);
          }}
        >
          {a.archived ? <ArchiveRestore size={16} /> : <Archive size={16} />}
        </ActionIcon>
      </Tooltip>
    );
  if (isMobile) {
    return (
      <Stack gap="xs">
        {assets.map((a) => (
          <Paper
            key={a.id}
            withBorder
            p="sm"
            radius="md"
            onClick={onOpen ? () => onOpen(a) : undefined}
            style={{ cursor: onOpen ? 'pointer' : undefined }}
          >
            <Group justify="space-between" wrap="nowrap">
              <Stack gap={0} style={{ minWidth: 0 }}>
                <Text size="sm" fw={500} truncate="end" c={a.archived ? 'dimmed' : undefined}>
                  {a.name}
                </Text>
                <Text size="xs" c="dimmed">
                  {a.asset_group} · {rate(a)}
                  {a.disposed_on ? ` · sold ${formatDate(a.disposed_on)}` : ''}
                </Text>
              </Stack>
              <Group gap="xs" wrap="nowrap">
                <MoneyText value={a.book_value} fw={500} />
                {archiveButton(a)}
              </Group>
            </Group>
          </Paper>
        ))}
      </Stack>
    );
  }
  return (
    <Table.ScrollContainer minWidth={640}>
      <Table highlightOnHover={!!onOpen} verticalSpacing="sm">
        <Table.Thead>
          <Table.Tr>
            <Table.Th>Asset</Table.Th>
            <Table.Th>Bought</Table.Th>
            <Table.Th>Rate</Table.Th>
            <Table.Th ta="right">Cost</Table.Th>
            <Table.Th ta="right">Book value</Table.Th>
            <Table.Th w={40} />
          </Table.Tr>
        </Table.Thead>
        <Table.Tbody>
          {assets.map((a) => (
            <Table.Tr key={a.id} onClick={onOpen ? () => onOpen(a) : undefined} style={{ cursor: onOpen ? 'pointer' : undefined }}>
              <Table.Td>
                <Group gap={6} wrap="nowrap">
                  <Text size="sm" fw={500} c={a.archived ? 'dimmed' : undefined}>
                    {a.name}
                  </Text>
                  {a.archived && (
                    <Badge color="gray" variant="outline">
                      Archived
                    </Badge>
                  )}
                  {a.disposed_on && (
                    <Badge color="gray" variant="light">
                      Sold
                    </Badge>
                  )}
                </Group>
                <Text size="xs" c="dimmed">
                  {a.asset_group}
                </Text>
              </Table.Td>
              <Table.Td>{formatDate(a.acquired_on)}</Table.Td>
              <Table.Td>{rate(a)}</Table.Td>
              <Table.Td ta="right">
                <MoneyText value={a.cost} />
              </Table.Td>
              <Table.Td ta="right">
                <MoneyText value={a.book_value} fw={500} />
              </Table.Td>
              <Table.Td>{archiveButton(a)}</Table.Td>
            </Table.Tr>
          ))}
        </Table.Tbody>
      </Table>
    </Table.ScrollContainer>
  );
}

function DepreciationSection() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const years = recentYearEnds(business.current_financial_year.end);
  const [yearEnd, setYearEnd] = useState(years[1] ?? years[0]!);
  const year = useDepreciationYear(yearEnd);
  const invalidate = yearEndChangeKeys(business.id);
  const post = useApiMutation(
    () =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/assets/depreciation', {
          params: { path: { business_id: business.id } },
          body: { year_end: yearEnd },
        }),
      ),
    { invalidate, successMessage: `Depreciation for the year ending ${formatDate(yearEnd)} posted` },
  );
  const remove = useApiMutation(
    () =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/assets/depreciation', {
          params: { path: { business_id: business.id }, query: { year_end: yearEnd } },
        }),
      ),
    { invalidate, successMessage: 'Depreciation removed' },
  );
  const posted = (year.data?.lines.length ?? 0) > 0;
  const confirmRemove = async () => {
    const ok = await confirmAction({
      title: 'Remove this year’s depreciation?',
      message: 'The year’s depreciation, and any later years’, is reversed. You can post it again.',
      confirmLabel: 'Remove',
      danger: true,
    });
    if (ok) remove.mutate();
  };

  let body: ReactNode;
  if (year.isPending) body = <ListSkeleton rows={3} height={36} />;
  else if (year.isError) body = <ErrorState error={year.error} onRetry={() => void year.refetch()} />;
  else if (!posted)
    body = (
      <Text size="sm" c="dimmed">
        Nothing posted for this year. Posting works out each active asset’s depreciation for the year from its value at the start of it.
      </Text>
    );
  else
    body = (
      <Table.ScrollContainer minWidth={isMobile ? 0 : 520}>
        <Table verticalSpacing="xs">
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Asset</Table.Th>
              {!isMobile && <Table.Th ta="right">Opening</Table.Th>}
              <Table.Th ta="right">Depreciation</Table.Th>
              <Table.Th ta="right">Closing</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {year.data.lines.map((line) => (
              <Table.Tr key={line.asset_id}>
                <Table.Td>{line.name}</Table.Td>
                {!isMobile && (
                  <Table.Td ta="right">
                    <MoneyText value={line.opening_value + line.purchases} />
                  </Table.Td>
                )}
                <Table.Td ta="right">
                  <MoneyText value={line.depreciation} />
                </Table.Td>
                <Table.Td ta="right">
                  <MoneyText value={line.closing_value} />
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
          <Table.Tfoot>
            <Table.Tr>
              <Table.Th>Total</Table.Th>
              {!isMobile && <Table.Th />}
              <Table.Th ta="right">
                <MoneyText value={year.data.depreciation} fw={600} />
              </Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Tfoot>
        </Table>
      </Table.ScrollContainer>
    );

  return (
    <Stack gap="sm">
      <Group justify="space-between" gap="sm" align="flex-end">
        <Select
          label="Financial year ending"
          data={years.map((y) => ({ value: y, label: formatDate(y) }))}
          value={yearEnd}
          onChange={(v) => v && setYearEnd(v)}
          allowDeselect={false}
          w={220}
        />
        {
          <Group gap="xs">
            {posted && (
              <Button variant="default" onClick={() => void confirmRemove()} loading={remove.isPending}>
                Remove
              </Button>
            )}
            <Button onClick={() => post.mutate()} loading={post.isPending}>
              {posted ? 'Post again' : 'Post depreciation'}
            </Button>
          </Group>
        }
      </Group>
      {body}
    </Stack>
  );
}

/** Fixed assets and their yearly depreciation. */
export function AssetsPanel() {
  const business = useCurrentBusiness();
  const isMobile = useIsMobile();
  const assets = useAssets();
  const [dialog, setDialog] = useState<{ opened: boolean; asset: Asset | null }>({ opened: false, asset: null });
  const close = () => setDialog((d) => ({ ...d, opened: false }));
  const largeAssets = (assets.data ?? []).filter((a) => !a.archived && a.cost > CHANGE_OF_USE_FROM);
  const archive = useApiMutation(
    (a: Asset) =>
      unwrap(
        api.POST('/api/v1/businesses/{business_id}/assets/{asset_id}/archive', {
          params: { path: { business_id: business.id, asset_id: a.id } },
          body: { archived: !a.archived },
        }),
      ),
    { invalidate: yearEndChangeKeys(business.id), successMessage: (row) => `${row.name} ${row.archived ? 'archived' : 'restored'}` },
  );
  const addButton = (
    <Button leftSection={<Plus size={16} />} onClick={() => setDialog({ opened: true, asset: null })}>
      Add asset
    </Button>
  );

  let list: ReactNode;
  if (assets.isPending) list = <ListSkeleton rows={4} />;
  else if (assets.isError) list = <ErrorState error={assets.error} onRetry={() => void assets.refetch()} />;
  else if (assets.data.length === 0)
    list = (
      <EmptyBlock
        icon={Laptop}
        title="No assets yet"
        description="Add equipment you depreciate rather than expense, such as a computer or camera. Its depreciation is posted once a year."
        action={addButton}
      />
    );
  else
    list = <AssetList assets={assets.data} onOpen={(asset) => setDialog({ opened: true, asset })} onArchive={(a) => archive.mutate(a)} />;

  return (
    <Stack gap="lg">
      <SectionCard
        title="Assets"
        description="Equipment you depreciate. The book value is after the latest depreciation posted."
        actions={!isMobile && (assets.data?.length ?? 0) > 0 && addButton}
        padding={isMobile ? 'sm' : 'lg'}
      >
        <Stack gap="md">
          {isMobile && (assets.data?.length ?? 0) > 0 && addButton}
          {list}
        </Stack>
      </SectionCard>
      {(assets.data?.length ?? 0) > 0 && (
        <SectionCard
          title="Depreciation"
          description="Post each financial year once its records are complete."
          padding={isMobile ? 'sm' : 'lg'}
        >
          <DepreciationSection />
        </SectionCard>
      )}
      {largeAssets.length > 0 && (
        <SectionCard
          title="GST change of use"
          description="Assets over $10,000 (excluding GST) are reviewed at each balance date for several years."
          padding={isMobile ? 'sm' : 'lg'}
        >
          <ChangeOfUse assets={largeAssets} />
        </SectionCard>
      )}
      <Modal opened={dialog.opened} onClose={close} title={dialog.asset ? 'Edit asset' : 'Add asset'} size="lg">
        <AssetForm key={dialog.asset?.id ?? 'new'} asset={dialog.asset} onDone={close} />
        {dialog.asset && <AssetDisposal key={`disposal-${dialog.asset.id}`} asset={dialog.asset} />}
      </Modal>
    </Stack>
  );
}
