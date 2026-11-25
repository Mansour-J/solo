import { Anchor, Text } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { findPendingAllocation } from './api';

/**
 * After money received is recorded or changed: if the business's rules proposed an allocation,
 * offers to review it. Best effort: the receipt is already saved, and anything pending is also
 * listed on the Allocations page.
 */
export async function offerAllocationReview(
  businessId: string,
  source: { invoice_payment_id: string } | { income_id: string },
): Promise<void> {
  try {
    const pending = await findPendingAllocation(businessId, source);
    if (!pending) return;
    notifications.show({
      color: 'blue',
      title: 'Plan where this money goes',
      autoClose: 12_000,
      message: (
        <Text size="sm">
          Your rules proposed how to split it.{' '}
          <Anchor href={`#/allocations?review=${pending.id}`} size="sm" onClick={() => notifications.clean()}>
            Review the allocation
          </Anchor>
        </Text>
      ),
    });
  } catch {
    // Nothing to do: pending allocations are listed on the Allocations page.
  }
}
