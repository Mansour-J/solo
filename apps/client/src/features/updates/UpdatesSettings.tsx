import { Alert, Button, Group, Paper, Progress, Stack, Text } from '@mantine/core';
import { useMutation } from '@tanstack/react-query';
import { CircleAlert, CircleCheck, Download, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import { EmptyBlock, ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { BRAND } from '@/branding/brand';
import { formatDate } from '@/lib/format';
import { checkForUpdate, installUpdate, isTauri, type AvailableUpdate, type InstallProgress } from '@/services/platform';
import { updateErrorMessage, useUpdateSupport } from './api';

function ProgressNote({ progress }: { progress: InstallProgress | null }) {
  if (progress?.event === 'installing') {
    return (
      <Stack gap={6}>
        <Text size="sm">Installing… {BRAND.name} will restart in a moment.</Text>
        <Progress value={100} animated aria-label="Installing the update" />
      </Stack>
    );
  }
  const percent = progress?.total ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100)) : null;
  return (
    <Stack gap={6}>
      <Text size="sm">{percent === null ? 'Downloading…' : `Downloading… ${percent}%`}</Text>
      <Progress value={percent ?? 100} animated={percent === null} aria-label="Downloading the update" />
    </Stack>
  );
}

function UpdateCard({
  update,
  installing,
  progress,
  onInstall,
}: {
  update: AvailableUpdate;
  installing: boolean;
  progress: InstallProgress | null;
  onInstall: () => void;
}) {
  return (
    <Paper withBorder radius="md" p="md">
      <Stack gap="sm">
        <Group gap="xs">
          <Download size={18} aria-hidden />
          <Text fw={600}>Version {update.version} is available</Text>
        </Group>
        {update.published_at !== null && (
          <Text size="sm" c="dimmed">
            Released {formatDate(new Date(update.published_at * 1000))}
          </Text>
        )}
        {update.notes && (
          <Text size="sm" style={{ whiteSpace: 'pre-wrap' }}>
            {update.notes}
          </Text>
        )}
        {installing ? (
          <ProgressNote progress={progress} />
        ) : (
          <>
            <Text size="sm" c="dimmed">
              {BRAND.name} downloads the update, checks it’s genuine, installs it and restarts. Your books aren’t changed. Your computer may
              ask for your password to install it.
            </Text>
            <Group>
              <Button leftSection={<RefreshCw size={16} />} onClick={onInstall}>
                Update and restart
              </Button>
            </Group>
          </>
        )}
      </Stack>
    </Paper>
  );
}

function Updates({ currentVersion }: { currentVersion: string }) {
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const check = useMutation({ mutationFn: checkForUpdate });
  const install = useMutation({
    mutationFn: () => {
      setProgress(null);
      return installUpdate(setProgress);
    },
  });
  const update = check.data ?? null;

  return (
    <SectionCard
      title="Updates"
      description={`New versions of ${BRAND.name} bring fixes and features. Check whenever you like; nothing is installed until you choose to.`}
    >
      <Stack gap="md">
        <Group justify="space-between" wrap="wrap" gap="sm">
          <Text size="sm">
            You’re using version <strong>{currentVersion}</strong>.
          </Text>
          <Button
            variant="default"
            leftSection={<RefreshCw size={16} />}
            loading={check.isPending}
            disabled={install.isPending}
            onClick={() => {
              install.reset();
              check.mutate();
            }}
          >
            Check for updates
          </Button>
        </Group>
        {check.isError && (
          <Alert color="red" icon={<CircleAlert size={18} />} title="Couldn’t check for updates">
            {updateErrorMessage(check.error)}
          </Alert>
        )}
        {check.isSuccess && !update && (
          <Group gap="xs" c="teal">
            <CircleCheck size={18} aria-hidden />
            <Text size="sm">You’re up to date: {currentVersion} is the latest version.</Text>
          </Group>
        )}
        {update && <UpdateCard update={update} installing={install.isPending} progress={progress} onInstall={() => install.mutate()} />}
        {install.isError && (
          <Alert color="red" icon={<CircleAlert size={18} />} title="The update wasn’t installed">
            {updateErrorMessage(install.error)}
          </Alert>
        )}
      </Stack>
    </SectionCard>
  );
}

/** Settings › Updates (desktop app): check for a new version, then update and restart. */
export function UpdatesSettings() {
  const support = useUpdateSupport();
  if (!isTauri()) {
    return (
      <EmptyBlock
        title="Only in the desktop app"
        description={`The web version of ${BRAND.name} is always the latest; there’s nothing to install.`}
      />
    );
  }
  if (support.isPending) return <ListSkeleton rows={2} />;
  if (support.isError || !support.data) return <ErrorState error={support.error} onRetry={() => void support.refetch()} />;
  if (!support.data.enabled) {
    return (
      <EmptyBlock
        title="This version doesn’t update itself"
        description={`You’re using version ${support.data.current_version}. Install new versions of ${BRAND.name} the way you installed this one.`}
      />
    );
  }
  return <Updates currentVersion={support.data.current_version} />;
}
