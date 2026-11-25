import { useMantineColorScheme } from '@mantine/core';
import type { Preferences, UserProfile } from '@solo/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { setFormatPreferences } from '@/lib/format';
import { api, unwrap } from '@/services/api/client';
import { useApiMutation } from '@/services/api/query';
import { accountKeys, useMe } from './api';

/** Applies the signed-in user's theme and formatting preferences whenever they load or change. */
export function usePreferenceSync() {
  const { data: user } = useMe();
  const { setColorScheme } = useMantineColorScheme();
  const theme = user?.preferences.theme;
  const dateFormat = user?.preferences.date_format;
  const numberLocale = user?.preferences.number_locale;
  useEffect(() => {
    if (theme) setColorScheme(theme);
  }, [theme, setColorScheme]);
  if (dateFormat && numberLocale) setFormatPreferences({ date_format: dateFormat, number_locale: numberLocale });
}

/** Saves preferences (and the name, which the endpoint updates together). */
export function useSavePreferences(successMessage?: string) {
  const queryClient = useQueryClient();
  return useApiMutation(
    ({ user, preferences }: { user: UserProfile; preferences: Preferences }) =>
      unwrap(api.PUT('/api/v1/users/me', { body: { first_name: user.first_name, last_name: user.last_name, preferences } })),
    {
      successMessage,
      onSuccess: (updated) => {
        queryClient.setQueryData(accountKeys.me, updated);
      },
    },
  );
}
