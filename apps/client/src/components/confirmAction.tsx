import { Text } from '@mantine/core';
import { modals } from '@mantine/modals';
import type { ReactNode } from 'react';

let counter = 0;

/** Promise-based confirmation for destructive or significant actions. */
export function confirmAction(options: { title: string; message: ReactNode; confirmLabel?: string; danger?: boolean }): Promise<boolean> {
  counter += 1;
  return new Promise((resolve) => {
    modals.openConfirmModal({
      modalId: `confirm-${counter}`,
      title: options.title,
      children: typeof options.message === 'string' ? <Text size="sm">{options.message}</Text> : options.message,
      labels: { confirm: options.confirmLabel ?? 'Confirm', cancel: 'Cancel' },
      confirmProps: { color: options.danger ? 'red' : undefined },
      onConfirm: () => resolve(true),
      onCancel: () => resolve(false),
      onClose: () => resolve(false),
    });
  });
}
