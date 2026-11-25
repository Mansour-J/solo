import { Box, Button, Center, Group, Paper, Skeleton, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { AlertTriangle, Inbox, RefreshCw, WifiOff, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';
import { errorMessage } from '@/services/api/errors';

export interface EmptyBlockProps {
  icon?: LucideIcon;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}

/** Explains an empty list and offers the next step. */
export function EmptyBlock({ icon: Icon = Inbox, title, description, action, compact }: EmptyBlockProps) {
  return (
    <Center py={compact ? 'lg' : 48} px="md">
      <Stack align="center" gap={compact ? 6 : 'sm'} maw={440} ta="center">
        <ThemeIcon size={compact ? 36 : 48} radius="xl" variant="light" color="brand">
          <Icon size={compact ? 18 : 22} strokeWidth={1.75} />
        </ThemeIcon>
        <Title order={compact ? 5 : 4}>{title}</Title>
        {description && (
          <Text size="sm" c="dimmed">
            {description}
          </Text>
        )}
        {action && <Box mt={4}>{action}</Box>}
      </Stack>
    </Center>
  );
}

export function ErrorState({ error, onRetry, title = 'Couldn’t load this' }: { error: unknown; onRetry?: () => void; title?: string }) {
  const offline = typeof navigator !== 'undefined' && !navigator.onLine;
  return (
    <Center py={48} px="md">
      <Stack align="center" gap="sm" maw={440} ta="center">
        <ThemeIcon size={48} radius="xl" variant="light" color={offline ? 'gray' : 'red'}>
          {offline ? <WifiOff size={22} /> : <AlertTriangle size={22} />}
        </ThemeIcon>
        <Title order={4}>{offline ? 'You’re offline' : title}</Title>
        <Text size="sm" c="dimmed">
          {offline ? 'Reconnect to the internet and try again.' : errorMessage(error)}
        </Text>
        {onRetry && (
          <Button variant="default" leftSection={<RefreshCw size={14} />} onClick={onRetry}>
            Try again
          </Button>
        )}
      </Stack>
    </Center>
  );
}

/** Stable-height skeleton rows to avoid layout shift while loading. */
export function ListSkeleton({ rows = 6, height = 52 }: { rows?: number; height?: number }) {
  return (
    <Stack gap="xs" aria-busy="true" aria-label="Loading">
      {Array.from({ length: rows }, (_, i) => (
        <Skeleton key={i} height={height} radius="md" />
      ))}
    </Stack>
  );
}

/** A titled surface for grouping related content on a page. */
export function SectionCard({
  title,
  description,
  actions,
  children,
  padding = 'lg',
}: {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  padding?: string;
}) {
  return (
    <Paper withBorder radius="lg" p={padding} bg="var(--app-surface-raised)">
      {(title || actions) && (
        <Group justify="space-between" align="flex-start" mb="md" gap="sm" wrap="nowrap">
          <Stack gap={2} style={{ minWidth: 0 }}>
            {title && <Title order={3}>{title}</Title>}
            {description && (
              <Text size="sm" c="dimmed">
                {description}
              </Text>
            )}
          </Stack>
          {actions && (
            <Group gap="xs" wrap="nowrap">
              {actions}
            </Group>
          )}
        </Group>
      )}
      {children}
    </Paper>
  );
}
