import { createQueryClient } from '@/services/api/query';

/** The app's single query cache (also used by the session store to reset on sign-out). */
export const queryClient = createQueryClient();
