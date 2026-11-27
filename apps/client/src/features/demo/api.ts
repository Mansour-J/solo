import type { BusinessSummary } from '@solo/api-client';
import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { businessKeys } from '@/features/businesses/api';
import { api, requestJson, unwrap } from '@/services/api/client';
import { useSessionStore } from '@/stores/session';
import { createDemoBusiness } from './demoData';

const CHOICE_KEY = 'first-run-choice';

/** What the owner chose on the first-run screen: explore the demo or set up their business. */
export type FirstRunChoice = 'demo' | 'setup';

export function rememberedChoice(): FirstRunChoice | null {
  try {
    const value = window.localStorage.getItem(CHOICE_KEY);
    return value === 'demo' || value === 'setup' ? value : null;
  } catch {
    return null;
  }
}

export function rememberChoice(choice: FirstRunChoice) {
  try {
    window.localStorage.setItem(CHOICE_KEY, choice);
  } catch {
    // Private browsing: the welcome screen just shows again.
  }
}

/** Deletes the demo business (there is at most one); returns the businesses left. */
async function deleteDemo(): Promise<BusinessSummary[]> {
  const list = await unwrap(api.GET('/api/v1/businesses'));
  for (const demo of list.filter((b) => b.is_demo)) {
    await unwrap(
      api.DELETE('/api/v1/businesses/{business_id}', {
        params: { path: { business_id: demo.id } },
        body: { confirmation_name: demo.name },
      }),
    );
  }
  return list.filter((b) => !b.is_demo);
}

/**
 * Builds, resets and removes the demo business. `progress` runs from 0 to 1 while it's being built
 * (null otherwise). Building replaces any demo there is, so it also resets one, or finishes one that
 * stopped part-way.
 */
export function useDemo() {
  const queryClient = useQueryClient();
  const selectBusiness = useSessionStore((s) => s.selectBusiness);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<unknown>(null);

  async function build(): Promise<boolean> {
    setError(null);
    setProgress(0);
    try {
      await deleteDemo();
      const id = await createDemoBusiness(requestJson, setProgress);
      rememberChoice('demo');
      selectBusiness(id);
      await queryClient.invalidateQueries();
      return true;
    } catch (e) {
      setError(e);
      await queryClient.invalidateQueries({ queryKey: businessKeys.all });
      return false;
    } finally {
      setProgress(null);
    }
  }

  /** Removes the demo and opens another business; returns false when none is left. */
  async function remove(): Promise<boolean> {
    const left = await deleteDemo();
    selectBusiness(left[0]?.id ?? null);
    await queryClient.invalidateQueries({ queryKey: businessKeys.all });
    return left.length > 0;
  }

  return { build, remove, progress, error };
}
