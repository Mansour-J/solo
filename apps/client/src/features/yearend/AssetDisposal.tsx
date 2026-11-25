import { Button, Group, Paper, SimpleGrid, Stack, Text, Textarea } from '@mantine/core';
import { DateInput } from '@mantine/dates';
import { useState } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { MoneyInput, MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { dateInputFormat, formatDate } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useDisposal, yearEndChangeKeys, type Asset, type Disposal } from './api';

function DisposalForm({ asset, disposal, onDone }: { asset: Asset; disposal: Disposal | null; onDone: () => void }) {
  const business = useCurrentBusiness();
  const [date, setDate] = useState<string | null>(disposal?.disposed_on ?? business.today);
  const [price, setPrice] = useState<number | null>(disposal?.sale_price ?? null);
  const [notes, setNotes] = useState(disposal?.notes ?? '');
  const save = useApiMutation(
    () =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/assets/{asset_id}/disposal', {
          params: { path: { business_id: business.id, asset_id: asset.id } },
          body: { disposed_on: date ?? '', sale_price: price ?? 0, notes: notes.trim() },
        }),
      ),
    { invalidate: yearEndChangeKeys(business.id), successMessage: `${asset.name} recorded as sold`, onSuccess: onDone },
  );
  return (
    <Stack gap="sm">
      <SimpleGrid cols={{ base: 1, sm: 2 }}>
        <DateInput label="Date sold or scrapped" valueFormat={dateInputFormat()} value={date} onChange={setDate} />
        <MoneyInput
          label="Amount received"
          description="Including any GST; 0 if scrapped."
          currency={business.currency}
          value={price}
          onChange={setPrice}
        />
      </SimpleGrid>
      <Textarea label="Notes" autosize minRows={1} maxLength={1000} value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
      <Group justify="flex-end">
        <Button variant="default" onClick={onDone}>
          Cancel
        </Button>
        <Button onClick={() => save.mutate()} loading={save.isPending} disabled={!date}>
          Record sale
        </Button>
      </Group>
    </Stack>
  );
}

function DisposalSummary({ asset, disposal, onChange }: { asset: Asset; disposal: Disposal; onChange: () => void }) {
  const business = useCurrentBusiness();
  const currency = business.currency;
  const undo = useApiMutation(
    () =>
      unwrap(
        api.DELETE('/api/v1/businesses/{business_id}/assets/{asset_id}/disposal', {
          params: { path: { business_id: business.id, asset_id: asset.id } },
        }),
      ),
    { invalidate: yearEndChangeKeys(business.id), successMessage: 'Sale undone' },
  );
  const confirmUndo = async () => {
    const ok = await confirmAction({
      title: `Undo the sale of ${asset.name}?`,
      message: 'The asset is held again and the sale is taken out of your books.',
      confirmLabel: 'Undo sale',
      danger: true,
    });
    if (ok) undo.mutate();
  };
  const line = (label: string, value: number) => (
    <Group justify="space-between" gap="xs">
      <Text size="sm" c="dimmed">
        {label}
      </Text>
      <MoneyText value={value} currency={currency} size="sm" />
    </Group>
  );
  return (
    <Stack gap={6}>
      <Text size="sm">
        Sold or scrapped on <strong>{formatDate(disposal.disposed_on)}</strong>. There’s no depreciation for that year.
      </Text>
      {line('Amount received', disposal.sale_price)}
      {disposal.sale_gst > 0 && line('GST on the sale', disposal.sale_gst)}
      {line('Value at the start of that year', disposal.book_value)}
      {disposal.recovered > 0 && line('Depreciation recovered (taxable income)', disposal.recovered)}
      {disposal.loss > 0 && line('Loss on disposal (deductible)', disposal.loss)}
      {disposal.capital_gain > 0 && line('Gain above cost (not taxable)', disposal.capital_gain)}
      <Group justify="flex-end" mt={4}>
        <Button variant="subtle" color="red" size="xs" onClick={() => void confirmUndo()} loading={undo.isPending}>
          Undo sale
        </Button>
        <Button variant="default" size="xs" onClick={onChange}>
          Change
        </Button>
      </Group>
    </Stack>
  );
}

/** In the asset dialog: record the asset as sold or scrapped, or show (and undo) the sale. */
export function AssetDisposal({ asset }: { asset: Asset }) {
  const sold = !!asset.disposed_on;
  const disposal = useDisposal(asset.id, sold);
  const [editing, setEditing] = useState(false);
  let content;
  if (sold && disposal.isPending) content = <ListSkeleton rows={2} />;
  else if (sold && disposal.isError) content = <ErrorState error={disposal.error} onRetry={() => void disposal.refetch()} />;
  else if (editing) content = <DisposalForm asset={asset} disposal={disposal.data ?? null} onDone={() => setEditing(false)} />;
  else if (sold && disposal.data) content = <DisposalSummary asset={asset} disposal={disposal.data} onChange={() => setEditing(true)} />;
  else
    content = (
      <Group justify="space-between" gap="sm">
        <Text size="sm" c="dimmed">
          Sold it or thrown it away? Record it to account for what you got back.
        </Text>
        <Button variant="default" size="xs" onClick={() => setEditing(true)}>
          Sell or scrap
        </Button>
      </Group>
    );
  return (
    <Paper withBorder radius="md" p="sm" mt="md">
      <Text size="sm" fw={600} mb={6}>
        Sold or scrapped
      </Text>
      {content}
    </Paper>
  );
}
