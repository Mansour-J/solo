// The calls into the desktop shell. The command names and argument names here must match the Rust side,
// whose tests call the same commands through IPC (apps/client/src-tauri/tests/updates.rs).
import type { InstallProgress } from './platform';

const { core, opener } = vi.hoisted(() => {
  class FakeChannel<T> {
    onmessage: (message: T) => void = () => undefined;
  }
  return {
    core: { invoke: vi.fn<(cmd: string, args?: Record<string, unknown>) => Promise<unknown>>(), Channel: FakeChannel },
    opener: { openUrl: vi.fn<(url: string) => Promise<void>>() },
  };
});
vi.mock('@tauri-apps/api/core', () => core);
vi.mock('@tauri-apps/plugin-opener', () => opener);

type TauriWindow = Window & { __TAURI_INTERNALS__?: object };

function asDesktop(desktop: boolean) {
  if (desktop) (window as TauriWindow).__TAURI_INTERNALS__ = {};
  else delete (window as TauriWindow).__TAURI_INTERNALS__;
}

/** A fresh copy of the module (`CLIENT_KIND` is decided when it loads). */
async function platform(desktop = true) {
  asDesktop(desktop);
  vi.resetModules();
  return import('./platform');
}

afterEach(() => {
  asDesktop(false);
  core.invoke.mockReset();
  opener.openUrl.mockReset();
  vi.useRealTimers();
});

describe('where the app runs', () => {
  it('knows the desktop app from a browser', async () => {
    const web = await platform(false);
    expect(web.isTauri()).toBe(false);
    expect(web.CLIENT_KIND).toBe('web');
    const desktop = await platform(true);
    expect(desktop.isTauri()).toBe(true);
    expect(desktop.CLIENT_KIND).toBe('desktop');
  });
});

describe('updates', () => {
  it('asks the desktop app whether it updates itself, and a browser never', async () => {
    const web = await platform(false);
    expect(await web.updateSupport()).toBeNull();
    expect(core.invoke).not.toHaveBeenCalled();

    const desktop = await platform(true);
    core.invoke.mockResolvedValueOnce({ enabled: true, current_version: '0.4.1' });
    expect(await desktop.updateSupport()).toEqual({ enabled: true, current_version: '0.4.1' });
    expect(core.invoke).toHaveBeenCalledWith('update_support');
  });

  it('checks the feed and passes on what it found or why it failed', async () => {
    const { checkForUpdate } = await platform();
    const found = { version: '0.5.0', current_version: '0.4.1', published_at: 1_795_685_400, notes: null };
    core.invoke.mockResolvedValueOnce(found).mockResolvedValueOnce(null).mockRejectedValueOnce('Couldn’t check for updates.');
    expect(await checkForUpdate()).toEqual(found);
    expect(await checkForUpdate()).toBeNull();
    await expect(checkForUpdate()).rejects.toBe('Couldn’t check for updates.');
    expect(core.invoke).toHaveBeenCalledWith('update_check');
  });

  it('installs with progress sent through a channel named onProgress', async () => {
    const { installUpdate } = await platform();
    const progress: InstallProgress[] = [];
    core.invoke.mockImplementationOnce(async (_cmd, args) => {
      const channel = args?.onProgress as InstanceType<typeof core.Channel<InstallProgress>>;
      channel.onmessage({ event: 'downloading', downloaded: 10, total: 20 });
      channel.onmessage({ event: 'installing' });
    });
    await installUpdate((p) => progress.push(p));
    expect(core.invoke).toHaveBeenCalledWith('update_install', { onProgress: expect.any(core.Channel) });
    expect(progress).toEqual([{ event: 'downloading', downloaded: 10, total: 20 }, { event: 'installing' }]);

    core.invoke.mockRejectedValueOnce('The download didn’t pass the security check.');
    await expect(installUpdate(() => undefined)).rejects.toBe('The download didn’t pass the security check.');
  });
});

describe('the local server', () => {
  it('reports its status in the desktop app, and nothing in a browser', async () => {
    const web = await platform(false);
    expect(await web.backendStatus()).toBeNull();
    expect(await web.waitForBackend()).toBeNull();

    const desktop = await platform(true);
    const ready = { state: 'ready', url: 'http://127.0.0.1:5000', key: 'k', error: null };
    core.invoke.mockResolvedValueOnce(ready);
    expect(await desktop.backendStatus()).toEqual(ready);
    expect(core.invoke).toHaveBeenCalledWith('backend_status');
  });

  it('waits while it starts', async () => {
    const { waitForBackend } = await platform();
    const ready = { state: 'ready', url: 'http://127.0.0.1:5000', key: 'k', error: null };
    core.invoke
      .mockResolvedValueOnce({ state: 'starting', url: null, key: null, error: null })
      .mockResolvedValueOnce({ state: 'starting', url: null, key: null, error: null })
      .mockResolvedValueOnce(ready);
    expect(await waitForBackend(1)).toEqual(ready);
    expect(core.invoke).toHaveBeenCalledTimes(3);
  });

  it('restarts the app', async () => {
    const { restartApp } = await platform();
    core.invoke.mockResolvedValueOnce(undefined);
    await restartApp();
    expect(core.invoke).toHaveBeenCalledWith('restart_app');
  });
});

describe('links and downloads', () => {
  it('opens links in the system browser on desktop, or a new tab', async () => {
    const desktop = await platform(true);
    await desktop.openExternal('https://example.com');
    expect(opener.openUrl).toHaveBeenCalledWith('https://example.com');

    const web = await platform(false);
    const open = vi.spyOn(window, 'open').mockReturnValue(null);
    await web.openExternal('https://example.com');
    expect(open).toHaveBeenCalledWith('https://example.com', '_blank', 'noopener,noreferrer');
    open.mockRestore();
  });

  it('saves a file as a download', async () => {
    vi.useFakeTimers();
    const { saveBlob } = await platform(false);
    const createObjectURL = vi.fn(() => 'blob:report');
    const revokeObjectURL = vi.fn();
    Object.assign(URL, { createObjectURL, revokeObjectURL });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => undefined);

    saveBlob(new Blob(['a,b']), 'report.csv');
    const link = click.mock.contexts[0] as HTMLAnchorElement;
    expect(link.download).toBe('report.csv');
    expect(link.href).toBe('blob:report');
    expect(link.isConnected).toBe(false);
    vi.advanceTimersByTime(1000);
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:report');
    click.mockRestore();
  });
});
