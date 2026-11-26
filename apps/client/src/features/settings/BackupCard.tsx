import { Alert, Button, FileButton, Group, Stack, Text } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Archive, RotateCcw } from 'lucide-react';
import { useState } from 'react';
import { confirmAction } from '@/components/confirmAction';
import { ErrorState, ListSkeleton, SectionCard } from '@/components/States';
import { formatDateTime } from '@/lib/format';
import { api, fetchBlob, unwrap, uploadFile } from '@/services/api/client';
import { errorMessage } from '@/services/api/errors';
import { isTauri, restartApp, saveBlob } from '@/services/platform';

const backupKeys = { status: ['me', 'backups'] as const };

interface RestoreResult {
  made_at: string;
  restart_required: boolean;
}

function filename() {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, '0');
  return `solo-backup-${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}.solobackup`;
}

/**
 * Settings › Your data › Back up and restore (SL-013): a backup file with every business and its
 * receipts, and a restore that replaces the books with one. Only the desktop app makes backups.
 */
export function BackupCard() {
  const queryClient = useQueryClient();
  const status = useQuery({ queryKey: backupKeys.status, queryFn: () => unwrap(api.GET('/api/v1/users/me/backups')) });
  const [busy, setBusy] = useState<'backup' | 'restore' | null>(null);
  const [restored, setRestored] = useState<RestoreResult | null>(null);

  if (status.isPending) return <ListSkeleton rows={2} />;
  if (status.isError) return <ErrorState error={status.error} onRetry={() => void status.refetch()} />;
  if (!status.data.available) return null;
  const last = status.data.last;

  async function backUp() {
    setBusy('backup');
    try {
      saveBlob(await fetchBlob('/api/v1/users/me/backups', 'POST'), filename());
      notifications.show({ color: 'teal', message: 'Backup saved. Keep a copy somewhere other than this computer.' });
      // The Business health check and the dashboard reminder read the last backup.
      await queryClient.invalidateQueries({ queryKey: backupKeys.status });
      await queryClient.invalidateQueries({ queryKey: ['business'] });
    } catch (error) {
      notifications.show({ color: 'red', title: 'Backup failed', message: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  }

  async function restore(file: File | null) {
    if (!file) return;
    const ok = await confirmAction({
      title: 'Restore this backup?',
      message: `Your books on this computer are replaced by the ones in ${file.name}, including receipts. A copy of your current books is kept in the app’s folder first.`,
      confirmLabel: 'Restore',
      danger: true,
    });
    if (!ok) return;
    setBusy('restore');
    try {
      const result = (await uploadFile('/api/v1/users/me/backups/restore', file, 'POST')) as RestoreResult;
      setRestored(result);
      if (isTauri()) await restartApp();
    } catch (error) {
      notifications.show({ color: 'red', title: 'Restore failed', message: errorMessage(error) });
    } finally {
      setBusy(null);
    }
  }

  return (
    <SectionCard
      title="Back up and restore"
      description="Your books are kept only on this computer. Back up regularly and keep the file somewhere else, such as a USB drive or a cloud folder."
    >
      <Stack gap="md">
        {restored ? (
          <Alert color="teal" title="Backup restored">
            Your books are as they were on {formatDateTime(restored.made_at)}.{' '}
            {isTauri() ? 'Solo is restarting to open them.' : 'Restart the local server to open them.'}
          </Alert>
        ) : (
          <Text size="sm">
            {last
              ? last.kind === 'restored'
                ? `Restored from a backup made ${formatDateTime(last.made_at)}.`
                : `Last backup: ${formatDateTime(last.made_at)}.`
              : 'You haven’t backed up your books yet.'}
          </Text>
        )}
        <Group>
          <Button leftSection={<Archive size={16} />} loading={busy === 'backup'} disabled={busy !== null} onClick={() => void backUp()}>
            Back up now
          </Button>
          <FileButton accept=".solobackup,application/octet-stream" onChange={(file) => void restore(file)}>
            {(props) => (
              <Button
                {...props}
                variant="default"
                leftSection={<RotateCcw size={16} />}
                loading={busy === 'restore'}
                disabled={busy !== null}
              >
                Restore from a backup
              </Button>
            )}
          </FileButton>
        </Group>
      </Stack>
    </SectionCard>
  );
}
