import { notifications } from '@mantine/notifications';
import { QueryClient, useMutation, useQueryClient, type QueryKey } from '@tanstack/react-query';
import { errorMessage, isApiError, type ApiError } from './errors';

export function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 15_000,
        gcTime: 5 * 60_000,
        refetchOnWindowFocus: true,
        retry: (count, error) =>
          !(isApiError(error) && ['FORBIDDEN', 'NOT_FOUND', 'UNAUTHENTICATED', 'SESSION_EXPIRED', 'VALIDATION'].includes(error.code)) &&
          count < 2,
      },
      mutations: { retry: false },
    },
  });
}

/** Mantine form methods the mutation helper needs to show server-side field errors. */
interface FormLike {
  setErrors: (errors: Record<string, string>) => void;
}

export interface ApiMutationOptions<TInput, TOutput> {
  /** Toast shown on success; omit for silent mutations. */
  successMessage?: string | ((output: TOutput, input: TInput) => string);
  /** Queries to refresh afterwards. */
  invalidate?: QueryKey[];
  /** Puts field errors from the server onto this form instead of a toast. */
  form?: FormLike;
  /** `false` to handle errors yourself. */
  errorToast?: boolean;
  onSuccess?: (output: TOutput, input: TInput) => void | Promise<void>;
  onError?: (error: ApiError) => void;
}

/** A mutation with consistent feedback: success toast, error toast or form field errors, refetching. */
export function useApiMutation<TInput, TOutput>(
  fn: (input: TInput) => Promise<TOutput>,
  options: ApiMutationOptions<TInput, TOutput> = {},
) {
  const queryClient = useQueryClient();
  return useMutation<TOutput, ApiError, TInput>({
    mutationFn: fn,
    onSuccess: async (output, input) => {
      await Promise.all((options.invalidate ?? []).map((queryKey) => queryClient.invalidateQueries({ queryKey })));
      if (options.successMessage) {
        const message = typeof options.successMessage === 'function' ? options.successMessage(output, input) : options.successMessage;
        notifications.show({ color: 'teal', message });
      }
      await options.onSuccess?.(output, input);
    },
    onError: (error) => {
      const hasFields = isApiError(error) && Object.keys(error.fields).length > 0;
      if (hasFields && options.form) {
        options.form.setErrors(error.fields);
      } else if (options.errorToast !== false) {
        notifications.show({ color: 'red', title: 'That didn’t work', message: errorMessage(error) });
      }
      options.onError?.(error);
    },
  });
}
