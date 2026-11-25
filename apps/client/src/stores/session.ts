// Whether the app has loaded the owner's profile, and the selected business. Server data (the profile,
// businesses) lives in the query cache; this store only tracks what the client itself decides.
import type { UserProfile } from '@solo/api-client';
import { create } from 'zustand';
import { queryClient } from '@/app/queryClient';
import { api, unwrap } from '@/services/api/client';

export type SessionStatus = 'loading' | 'ready';

interface SessionState {
  status: SessionStatus;
  currentBusinessId: string | null;
  bootstrap: () => Promise<void>;
  selectBusiness: (businessId: string | null) => void;
}

export const ME_KEY = ['me'] as const;

const BUSINESS_KEY = 'selected-business';

function rememberedBusiness(): string | null {
  try {
    return window.localStorage.getItem(BUSINESS_KEY);
  } catch {
    return null;
  }
}

export const useSessionStore = create<SessionState>((set, get) => ({
  status: 'loading',
  currentBusinessId: null,

  async bootstrap() {
    const user: UserProfile = await unwrap(api.GET('/api/v1/users/me'));
    queryClient.setQueryData(ME_KEY, user);
    set({ status: 'ready', currentBusinessId: rememberedBusiness() });
  },

  selectBusiness(businessId) {
    try {
      if (businessId) window.localStorage.setItem(BUSINESS_KEY, businessId);
      else window.localStorage.removeItem(BUSINESS_KEY);
    } catch {
      // Private browsing: the choice just isn't remembered.
    }
    if (get().currentBusinessId !== businessId) set({ currentBusinessId: businessId });
  },
}));
