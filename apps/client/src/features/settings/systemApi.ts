import type { components } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';

export type EmailSettingsView = components['schemas']['EmailSettings'];
export type SmtpSecurity = components['schemas']['SmtpSecurity'];
export type SystemStatus = components['schemas']['SystemStatus'];
export type StatusCheck = components['schemas']['StatusCheck'];

export const emailSettingsKeys = { all: ['me', 'email-settings'] as const };
export const systemKeys = { status: ['me', 'system-status'] as const };

export function useEmailSettings() {
  return useQuery({ queryKey: emailSettingsKeys.all, queryFn: () => unwrap(api.GET('/api/v1/users/me/email-settings')) });
}

/** The server's checks of the database, file storage, PDF documents and email. */
export function useSystemStatus() {
  return useQuery({ queryKey: systemKeys.status, queryFn: () => unwrap(api.GET('/api/v1/users/me/system-status')) });
}
