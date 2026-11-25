// API errors as the UI sees them. The server returns `{ error: { code, message, fields } }`.

export type ApiErrorCode =
  | 'VALIDATION'
  | 'BAD_REQUEST'
  | 'UNAUTHENTICATED'
  | 'TOKEN_EXPIRED'
  | 'SESSION_EXPIRED'
  | 'INVALID_CREDENTIALS'
  | 'FORBIDDEN'
  | 'NOT_FOUND'
  | 'CONFLICT'
  | 'RATE_LIMITED'
  | 'PAYLOAD_TOO_LARGE'
  | 'SERVICE_UNAVAILABLE'
  | 'INTERNAL'
  | 'NETWORK';

export class ApiError extends Error {
  readonly code: ApiErrorCode;
  readonly status: number;
  /** Field-level messages keyed by request field name (`address.city`). */
  readonly fields: Record<string, string>;

  constructor(code: ApiErrorCode, message: string, status = 0, fields: Record<string, string> = {}) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.status = status;
    this.fields = fields;
  }

  static fromResponse(status: number, body: unknown): ApiError {
    const error = (body as { error?: { code?: string; message?: string; fields?: Record<string, string> } } | null)?.error;
    if (error?.code && error.message) return new ApiError(error.code as ApiErrorCode, error.message, status, error.fields ?? {});
    if (status >= 500) return new ApiError('INTERNAL', 'Something went wrong on our side. Please try again.', status);
    return new ApiError('BAD_REQUEST', `The request failed (${status}).`, status);
  }
}

export function isApiError(error: unknown): error is ApiError {
  return error instanceof ApiError;
}

export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  return 'Something went wrong. Please try again.';
}
