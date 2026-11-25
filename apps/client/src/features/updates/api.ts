import { useQuery } from '@tanstack/react-query';
import { isTauri, updateSupport } from '@/services/platform';

export const updateKeys = {
  support: ['desktop', 'updates'] as const,
};

/** Whether this desktop build updates itself, and its version (fixed for the life of the app). */
export function useUpdateSupport() {
  return useQuery({ queryKey: updateKeys.support, queryFn: updateSupport, enabled: isTauri(), staleTime: Infinity });
}

/** Turns a rejected native call into the message to show (commands reject with a plain string). */
export function updateErrorMessage(error: unknown): string {
  if (typeof error === 'string' && error) return error;
  if (error instanceof Error && error.message) return error.message;
  return 'Something went wrong. Try again.';
}
