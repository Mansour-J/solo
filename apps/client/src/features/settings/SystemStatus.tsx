import { Anchor, Badge, Button, Group, Paper, Stack, Text } from '@mantine/core';
import { RefreshCw } from 'lucide-react';
import { Link } from 'react-router';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { useServerInfo } from '@/features/server/api';
import { useUpdateSupport } from '@/features/updates/api';
import { isTauri } from '@/services/platform';
import { useSystemStatus, type StatusCheck } from './systemApi';

const BADGES = {
  ok: { color: 'teal', label: 'Working' },
  warning: { color: 'orange', label: 'Check this' },
  error: { color: 'red', label: 'Not working' },
} as const;

function Check({ check }: { check: Pick<StatusCheck, 'name' | 'status' | 'detail' | 'action' | 'link'> }) {
  const badge = BADGES[check.status];
  return (
    <Paper withBorder radius="md" p="sm">
      <Group justify="space-between" align="flex-start" wrap="nowrap" gap="sm">
        <Stack gap={2} style={{ minWidth: 0 }}>
          <Text fw={600}>{check.name}</Text>
          <Text size="sm">{check.detail}</Text>
          {check.action && (
            <Text size="sm" c="dimmed">
              {check.action}{' '}
              {check.link && (
                <Anchor component={Link} to={check.link} size="sm">
                  Open settings
                </Anchor>
              )}
            </Text>
          )}
        </Stack>
        <Badge color={badge.color} variant="light" style={{ flexShrink: 0 }}>
          {badge.label}
        </Badge>
      </Group>
    </Paper>
  );
}

/** The desktop app's updates: whether this copy can update itself. */
function UpdatesCheck() {
  const support = useUpdateSupport();
  if (!isTauri()) return null;
  const enabled = support.data?.enabled ?? false;
  return (
    <Check
      check={{
        name: 'Updates',
        status: enabled ? 'ok' : 'warning',
        detail: enabled
          ? 'This copy can check for and install new versions.'
          : 'This copy doesn’t update itself: new versions come as a new installer.',
        action: enabled ? null : 'Install new versions from the installer when they’re released.',
        link: null,
      }}
    />
  );
}

/** Settings › System status: whether each part of the app works here, and what to do if not. */
export function SystemStatusPage() {
  const status = useSystemStatus();
  const info = useServerInfo();
  let content;
  if (status.isPending) content = <ListSkeleton rows={4} />;
  else if (status.isError) content = <ErrorState error={status.error} onRetry={() => void status.refetch()} />;
  else
    content = (
      <Stack gap="sm">
        {status.data.checks.map((c) => (
          <Check key={c.key} check={c} />
        ))}
        <UpdatesCheck />
        <Text size="xs" c="dimmed">
          Version {status.data.version} · database version {status.data.schema_version}
          {info.data ? ` · ${info.data.brand.name}` : ''}
        </Text>
      </Stack>
    );
  return (
    <SectionCard
      title="System status"
      description="Checks each part of the app on this computer: if something isn’t working, it says what to do."
      actions={
        <Button
          variant="default"
          size="xs"
          leftSection={<RefreshCw size={14} />}
          loading={status.isFetching}
          onClick={() => void status.refetch()}
        >
          Check again
        </Button>
      }
    >
      {content}
    </SectionCard>
  );
}
