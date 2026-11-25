// The destination form: values, validation (mirroring the API) and conversion to the API shape.
import { z } from 'zod';
import type { Destination, DestinationInput, DestinationKind } from './api';

export interface DestinationFormValues {
  name: string;
  kind: DestinationKind;
  /** Minor units, or null for none. */
  target_amount: number | null;
  /** Minor units, or null for none. */
  minimum_balance: number | null;
  notes: string;
}

export function emptyDestinationValues(): DestinationFormValues {
  return { name: '', kind: 'custom', target_amount: null, minimum_balance: null, notes: '' };
}

export function fromDestination(destination: Destination): DestinationFormValues {
  return {
    name: destination.name,
    kind: destination.kind as DestinationKind,
    target_amount: destination.target_amount ?? null,
    minimum_balance: destination.minimum_balance ?? null,
    notes: destination.notes,
  };
}

const optionalAmount = z
  .number()
  .nullable()
  .refine((v) => v === null || v > 0, 'Enter an amount greater than zero, or leave it empty.');

export const destinationSchema = z
  .object({
    name: z.string().trim().min(1, 'Enter a name.').max(80, 'Use 80 characters or fewer.'),
    target_amount: optionalAmount,
    minimum_balance: optionalAmount,
    notes: z.string().max(1000, 'Use 1,000 characters or fewer.'),
  })
  .loose();

export function toDestinationInput(values: DestinationFormValues): DestinationInput {
  return {
    name: values.name.trim(),
    kind: values.kind,
    target_amount: values.target_amount,
    minimum_balance: values.minimum_balance,
    notes: values.notes.trim(),
  };
}
