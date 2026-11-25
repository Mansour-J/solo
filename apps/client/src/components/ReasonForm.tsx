import { Button, Group, Stack, Text, Textarea } from '@mantine/core';
import { useState, type ReactNode } from 'react';

/** The body of the reason prompt (see promptReason). */
export function ReasonForm({
  message,
  label,
  required,
  confirmLabel,
  danger,
  onDone,
}: {
  message: ReactNode;
  label: string;
  required: boolean;
  confirmLabel: string;
  danger: boolean;
  onDone: (reason: string | null) => void;
}) {
  const [reason, setReason] = useState('');
  const [error, setError] = useState<string | null>(null);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (required && !reason.trim()) {
          setError('Please give a reason. It’s kept in the history.');
          return;
        }
        onDone(reason.trim());
      }}
    >
      <Stack>
        {typeof message === 'string' ? <Text size="sm">{message}</Text> : message}
        <Textarea
          label={label}
          data-autofocus
          value={reason}
          maxLength={500}
          error={error}
          onChange={(e) => setReason(e.currentTarget.value)}
        />
        <Group justify="flex-end">
          <Button variant="default" onClick={() => onDone(null)}>
            Cancel
          </Button>
          <Button type="submit" color={danger ? 'red' : undefined}>
            {confirmLabel}
          </Button>
        </Group>
      </Stack>
    </form>
  );
}
