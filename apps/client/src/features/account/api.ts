import type { UserProfile } from '@solo/api-client';
import { useQuery } from '@tanstack/react-query';
import { api, unwrap } from '@/services/api/client';
import { ME_KEY } from '@/stores/session';

export const accountKeys = {
  me: ME_KEY,
};

export function useMe() {
  return useQuery({ queryKey: accountKeys.me, queryFn: () => unwrap(api.GET('/api/v1/users/me')) });
}

/** The owner's profile. Loaded before any route renders (`App.tsx`), so it is always there. */
export function useCurrentUser(): UserProfile {
  const { data } = useMe();
  if (!data) throw new Error('useCurrentUser used before the profile loaded');
  return data;
}
