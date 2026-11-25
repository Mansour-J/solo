import { Button, Center, Loader, MantineProvider, Stack, Text } from '@mantine/core';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import { QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useState, type ReactNode } from 'react';
import { RouterProvider } from 'react-router/dom';
import { BRAND } from '@/branding/brand';
import { LogoMark } from '@/components/Logo';
import { usePreferenceSync } from '@/features/account/preferences';
import { BackendUnavailable, prepareBackend } from '@/features/server/api';
import { errorMessage } from '@/services/api/errors';
import { isTauri, restartApp, type BackendStatus } from '@/services/platform';
import { useSessionStore } from '@/stores/session';
import { theme } from '@/theme/theme';
import { queryClient } from './queryClient';
import { createAppRouter } from './router';

function Splash({ error, onRetry, message }: { error?: string; onRetry?: () => void; message?: string }) {
  return (
    <Center h="100dvh">
      <Stack align="center" gap="sm" maw={360} ta="center" px="md">
        <LogoMark size={44} />
        {message && !error && (
          <Text size="sm" c="dimmed">
            {message}
          </Text>
        )}
        {error ? (
          <>
            <Text c="red" size="sm">
              {BRAND.name} couldn’t start: {error}
            </Text>
            {onRetry && (
              <Text size="sm" c="brand" style={{ cursor: 'pointer' }} onClick={onRetry}>
                Try again
              </Text>
            )}
          </>
        ) : (
          <Loader size="sm" type="dots" aria-label="Loading" />
        )}
      </Stack>
    </Center>
  );
}

function SyncPreferences() {
  usePreferenceSync();
  return null;
}

/**
 * Applies the owner's theme and formats, once the profile has loaded: before it, the desktop app
 * doesn't know its server yet.
 */
function PreferenceSync() {
  const ready = useSessionStore((s) => s.status === 'ready');
  return ready ? <SyncPreferences /> : null;
}

/** The desktop app's server couldn't start: say so, and offer to try again. */
function BackendProblem({ status }: { status: BackendStatus }) {
  return (
    <Center h="100dvh">
      <Stack align="center" gap="sm" maw={420} ta="center" px="md">
        <LogoMark size={44} />
        <Text fw={600}>Your books couldn’t be opened</Text>
        <Text size="sm" c="dimmed">
          {status.error}
        </Text>
        <Button variant="default" onClick={() => void restartApp()}>
          Restart and try again
        </Button>
      </Stack>
    </Center>
  );
}

/**
 * On desktop, waits for the app's server first (it takes a few seconds the first time). Then loads the
 * owner's profile before rendering any route.
 */
function Boot({ children }: { children: ReactNode }) {
  const status = useSessionStore((s) => s.status);
  const bootstrap = useSessionStore((s) => s.bootstrap);
  const [error, setError] = useState<string | null>(null);
  const [backendProblem, setBackendProblem] = useState<BackendStatus | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    prepareBackend()
      .then(() => bootstrap())
      .catch((e: unknown) => {
        if (cancelled) return;
        if (e instanceof BackendUnavailable) setBackendProblem(e.status);
        else setError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [bootstrap, attempt]);

  if (backendProblem) return <BackendProblem status={backendProblem} />;
  if (error)
    return (
      <Splash
        error={error}
        onRetry={() => {
          setError(null);
          setAttempt((a) => a + 1);
        }}
      />
    );
  if (status === 'loading') return <Splash message={isTauri() ? 'Opening your books…' : undefined} />;
  return <>{children}</>;
}

export function App() {
  const [router] = useState(createAppRouter);
  return (
    <MantineProvider theme={theme} defaultColorScheme="auto">
      <QueryClientProvider client={queryClient}>
        <ModalsProvider>
          <Notifications position="top-right" limit={4} zIndex={1000} />
          <PreferenceSync />
          <Boot>
            <RouterProvider router={router} />
          </Boot>
        </ModalsProvider>
      </QueryClientProvider>
    </MantineProvider>
  );
}
