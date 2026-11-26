import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import type * as Platform from '@/services/platform';
import { BackupCard } from './BackupCard';

const { saveBlob } = vi.hoisted(() => ({ saveBlob: vi.fn() }));
vi.mock('@/services/platform', async (importOriginal) => ({ ...(await importOriginal<typeof Platform>()), saveBlob }));

afterEach(() => {
  vi.restoreAllMocks();
  saveBlob.mockReset();
});

type Status = { available: boolean; last: null | { kind: string; made_at: string; size_bytes: number }; recent: unknown[] };

function serve(status: Status, restore: () => Response = () => Response.json({ made_at: '2026-11-20T01:00:00Z', restart_required: true })) {
  const requests: { method: string; url: string }[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    requests.push({ method: request.method, url: request.url });
    if (request.url.endsWith('/backups/restore')) return restore();
    if (request.method === 'POST') return new Response('backup', { status: 200 });
    return Response.json(status);
  });
  return requests;
}

describe('back up and restore', () => {
  it('isn’t shown where backups can’t be made', async () => {
    const requests = serve({ available: false, last: null, recent: [] });
    renderWithProviders(<BackupCard />, { path: '/settings/data' });
    await waitFor(() => expect(requests).toHaveLength(1));
    await waitFor(() => expect(screen.queryByRole('progressbar')).not.toBeInTheDocument());
    expect(screen.queryByText('Back up and restore')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Back up now' })).not.toBeInTheDocument();
  });

  it('backs up and saves the file', async () => {
    const requests = serve({ available: true, last: null, recent: [] });
    renderWithProviders(<BackupCard />, { path: '/settings/data' });
    expect(await screen.findByText('You haven’t backed up your books yet.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Back up now' }));
    await waitFor(() => expect(saveBlob).toHaveBeenCalledTimes(1));
    expect(saveBlob.mock.calls[0]?.[1]).toMatch(/^solo-backup-\d{4}-\d{2}-\d{2}\.solobackup$/);
    expect(requests.some((r) => r.method === 'POST' && r.url.endsWith('/api/v1/users/me/backups'))).toBe(true);
  });

  it('shows the last backup', async () => {
    serve({ available: true, last: { kind: 'backup', made_at: '2026-11-20T01:00:00Z', size_bytes: 1000 }, recent: [] });
    renderWithProviders(<BackupCard />, { path: '/settings/data' });
    expect(await screen.findByText(/^Last backup:/)).toBeInTheDocument();
  });

  it('restores a backup after confirming, and says so', async () => {
    const requests = serve({ available: true, last: null, recent: [] });
    renderWithProviders(<BackupCard />, { path: '/settings/data' });
    await screen.findByRole('button', { name: 'Back up now' });
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    await userEvent.upload(input, new File(['x'], 'books.solobackup'));
    const dialog = await screen.findByRole('dialog', { name: 'Restore this backup?' });
    expect(within(dialog).getByText(/books\.solobackup/)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Restore' }));
    expect(await screen.findByText('Backup restored')).toBeInTheDocument();
    expect(screen.getByText(/Restart the local server to open them/)).toBeInTheDocument();
    expect(requests.filter((r) => r.url.endsWith('/backups/restore'))).toHaveLength(1);
  });

  it('says why a file can’t be restored', async () => {
    serve({ available: true, last: null, recent: [] }, () =>
      Response.json(
        {
          error: {
            code: 'validation_failed',
            message: 'Validation failed',
            fields: { file: ['This isn’t a Solo backup file, or it’s damaged.'] },
          },
        },
        { status: 422 },
      ),
    );
    renderWithProviders(<BackupCard />, { path: '/settings/data' });
    await screen.findByRole('button', { name: 'Back up now' });
    await userEvent.upload(document.querySelector('input[type="file"]') as HTMLInputElement, new File(['x'], 'notes.solobackup'));
    await userEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Restore' }));
    expect(await screen.findByText('Restore failed')).toBeInTheDocument();
    expect(screen.queryByText('Backup restored')).not.toBeInTheDocument();
  });
});
