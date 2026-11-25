import { modals } from '@mantine/modals';
import type { ReactNode } from 'react';
import { ReasonForm } from './ReasonForm';

let counter = 0;

/** Asks for a reason before a significant change (cancel an invoice, delete a payment). Null if cancelled. */
export function promptReason(options: {
  title: string;
  message: ReactNode;
  label?: string;
  required?: boolean;
  confirmLabel?: string;
  danger?: boolean;
}): Promise<string | null> {
  counter += 1;
  const modalId = `reason-${counter}`;
  return new Promise((resolve) => {
    let settled = false;
    const done = (value: string | null) => {
      if (settled) return;
      settled = true;
      modals.close(modalId);
      resolve(value);
    };
    modals.open({
      modalId,
      title: options.title,
      onClose: () => done(null),
      children: (
        <ReasonForm
          message={options.message}
          label={options.label ?? 'Reason'}
          required={options.required ?? false}
          confirmLabel={options.confirmLabel ?? 'Confirm'}
          danger={options.danger ?? false}
          onDone={done}
        />
      ),
    });
  });
}
