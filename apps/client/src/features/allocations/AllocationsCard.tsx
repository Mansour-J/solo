import { Anchor, Stack, Text } from '@mantine/core';
import { Link } from 'react-router';
import { useAllocationBalances } from './api';
import { BalancesPanel } from './BalancesPanel';

/** Dashboard card: the largest allocation balances and what's waiting for review. Hidden until allocations are used. */
export function AllocationsCard() {
  const balances = useAllocationBalances();
  const data = balances.data;
  if (!data || (!data.has_rules && data.total === 0 && data.pending_count === 0)) return null;
  return (
    <Stack gap="xs">
      <BalancesPanel
        balances={data}
        loading={balances.isPending}
        title="Allocations"
        limit={5}
        actions={
          <Anchor component={Link} to="/allocations" size="sm">
            View all
          </Anchor>
        }
      />
      {data.pending_count > 0 && (
        <Text size="sm">
          <Anchor component={Link} to="/allocations">
            {data.pending_count} {data.pending_count === 1 ? 'payment' : 'payments'} to review
          </Anchor>
        </Text>
      )}
    </Stack>
  );
}
