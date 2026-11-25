import { Badge, Button, Group, Stack, Table, Text, TextInput } from '@mantine/core';
import { useState } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { MoneyText } from '@/components/Money';
import { ErrorState, ListSkeleton } from '@/components/States';
import { useCurrentBusiness } from '@/features/businesses/api';
import { formatDate, formatPercent } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { useAssetUse, yearEndChangeKeys, type Asset, type AssetUse } from './api';

const percent = (value: string | null | undefined) => (value == null ? '—' : formatPercent(Number(value) / 100, 1));

function ReviewInput({ asset, periodEnd }: { asset: Asset; periodEnd: string }) {
  const business = useCurrentBusiness();
  const [value, setValue] = useState('');
  const [error, setError] = useState<string | null>(null);
  const save = useApiMutation(
    (business_use: string) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/assets/{asset_id}/use-reviews', {
          params: { path: { business_id: business.id, asset_id: asset.id } },
          body: { period_end: periodEnd, business_use },
        }),
      ),
    { invalidate: yearEndChangeKeys(business.id), successMessage: 'Business use saved' },
  );
  const submit = () => {
    const number = Number(value.trim().replace(/%$/, ''));
    if (value.trim() === '' || !Number.isFinite(number) || number < 0 || number > 100) {
      setError('Enter a percentage between 0 and 100.');
      return;
    }
    setError(null);
    save.mutate(String(number));
  };
  return (
    <Group gap="xs" wrap="nowrap" align="flex-start">
      <TextInput
        aria-label={`Business use to ${formatDate(periodEnd)}`}
        placeholder="e.g. 80"
        size="xs"
        w={90}
        inputMode="decimal"
        rightSection={<Text size="xs">%</Text>}
        value={value}
        onChange={(e) => setValue(e.currentTarget.value)}
        error={error}
      />
      <Button size="xs" onClick={submit} loading={save.isPending}>
        Save
      </Button>
    </Group>
  );
}

function AssetPeriods({ asset, use }: { asset: Asset; use: AssetUse }) {
  const business = useCurrentBusiness();
  const remove = useApiMutation(
    (period_end: string) =>
      unwrap(
        api.PUT('/api/v1/businesses/{business_id}/assets/{asset_id}/use-reviews', {
          params: { path: { business_id: business.id, asset_id: asset.id } },
          body: { period_end, business_use: null },
        }),
      ),
    { invalidate: yearEndChangeKeys(business.id), successMessage: 'Review removed' },
  );
  const confirmRemove = async (periodEnd: string) => {
    const ok = await confirmAction({
      title: 'Remove this review?',
      message: 'This period’s business use and any later ones are removed, and their GST adjustments reversed.',
      confirmLabel: 'Remove',
      danger: true,
    });
    if (ok) remove.mutate(periodEnd);
  };
  return (
    <Stack gap="xs">
      <Group justify="space-between" align="baseline" gap="xs">
        <Text fw={600}>{asset.name}</Text>
        <Text size="sm" c="dimmed">
          GST on the cost <MoneyText value={use.full_gst} currency={business.currency} size="sm" inherit /> · intended business use{' '}
          {percent(use.intended_use)} · {use.periods.length} adjustment periods
        </Text>
      </Group>
      <Table.ScrollContainer minWidth={640}>
        <Table verticalSpacing={6}>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Period to</Table.Th>
              <Table.Th>Business use</Table.Th>
              <Table.Th>Since bought</Table.Th>
              <Table.Th ta="right">GST adjustment</Table.Th>
              <Table.Th />
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {use.periods.map((p) => (
              <Table.Tr key={p.period_end}>
                <Table.Td>
                  {formatDate(p.period_end)}
                  <Text size="xs" c="dimmed">
                    {p.months} months
                  </Text>
                </Table.Td>
                <Table.Td>
                  {p.business_use != null ? (
                    percent(p.business_use)
                  ) : p.reviewable ? (
                    <ReviewInput asset={asset} periodEnd={p.period_end} />
                  ) : (
                    <Text size="sm" c="dimmed">
                      {p.ended ? 'Review the earlier period first' : `After ${formatDate(p.period_end)}`}
                    </Text>
                  )}
                </Table.Td>
                <Table.Td>{p.actual_use != null ? `${percent(p.actual_use)} (was ${percent(p.previous_use)})` : '—'}</Table.Td>
                <Table.Td ta="right">
                  {p.adjustment == null ? (
                    '—'
                  ) : p.adjustment === 0 ? (
                    <Text size="sm" c="dimmed">
                      None needed
                    </Text>
                  ) : (
                    <Group gap={6} justify="flex-end" wrap="nowrap">
                      <Badge size="xs" variant="light" color={p.adjustment > 0 ? 'teal' : 'orange'}>
                        {p.adjustment > 0 ? 'Claim' : 'Pay back'}
                      </Badge>
                      <MoneyText value={Math.abs(p.adjustment)} currency={business.currency} size="sm" />
                    </Group>
                  )}
                </Table.Td>
                <Table.Td ta="right">
                  {p.business_use != null && (
                    <Button size="compact-xs" variant="subtle" color="red" onClick={() => void confirmRemove(p.period_end)}>
                      Remove
                    </Button>
                  )}
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Table.ScrollContainer>
    </Stack>
  );
}

function AssetChangeOfUse({ asset }: { asset: Asset }) {
  const use = useAssetUse(asset.id);
  if (use.isPending) return <ListSkeleton rows={2} />;
  if (use.isError) return <ErrorState error={use.error} onRetry={() => void use.refetch()} />;
  if (!use.data.applies) {
    return (
      <Text size="sm" c="dimmed">
        <Text span fw={600} c="var(--mantine-color-text)">
          {asset.name}:
        </Text>{' '}
        {use.data.reason}
      </Text>
    );
  }
  return <AssetPeriods asset={asset} use={use.data} />;
}

/**
 * Year end › Assets › GST change of use: for each asset over $10,000, its business use at each
 * balance date and the GST adjustment that follows (IR375).
 */
export function ChangeOfUse({ assets }: { assets: Asset[] }) {
  return (
    <Stack gap="lg">
      <Text size="sm">
        At each balance date, enter how much the asset was used for the business during that year (from a logbook or diary). If its use
        since you bought it differs from what you claimed by 10 percentage points or more, or by over $1,000 of GST, the difference is
        claimed or paid back in the GST return that includes the balance date.
      </Text>
      {assets.map((asset) => (
        <AssetChangeOfUse key={asset.id} asset={asset} />
      ))}
    </Stack>
  );
}
