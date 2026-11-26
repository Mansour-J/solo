import type * as Platform from '@/services/platform';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { AvailableUpdate, InstallProgress, UpdateSupport } from '@/services/platform';
import { SettingsLayout } from '@/features/settings/SettingsLayout';
import { renderWithProviders } from '@/test/render';
import { updateErrorMessage } from './api';
import { UpdatesSettings } from './UpdatesSettings';

const { native } = vi.hoisted(() => ({
  native: {
    desktop: true,
    support: { enabled: true, current_version: '0.3.0' } as UpdateSupport | null,
    /** Errors the next support queries fail with. */
    supportFailures: [] as Error[],
    check: (async () => null) as () => Promise<AvailableUpdate | null>,
    install: (async () => undefined) as (onProgress: (p: InstallProgress) => void) => Promise<void>,
  },
}));
vi.mock('@/services/platform', async (importOriginal) => ({
  ...(await importOriginal<typeof Platform>()),
  isTauri: () => native.desktop,
  updateSupport: async () => {
    const failure = native.supportFailures.shift();
    if (failure) throw failure;
    return native.desktop ? native.support : null;
  },
  checkForUpdate: () => native.check(),
  installUpdate: (onProgress: (p: InstallProgress) => void) => native.install(onProgress),
}));

afterEach(() => {
  native.desktop = true;
  native.support = { enabled: true, current_version: '0.3.0' };
  native.supportFailures = [];
  native.check = async () => null;
  native.install = async () => undefined;
});

const newer: AvailableUpdate = {
  version: '0.4.0',
  current_version: '0.3.0',
  published_at: Date.UTC(2026, 11, 1) / 1000,
  notes: 'Faster reports.\nA new tax summary.',
};

describe('checking for updates', () => {
  it('says when the app is up to date', async () => {
    renderWithProviders(<UpdatesSettings />);
    expect(await screen.findByText(/You’re using version/)).toHaveTextContent('0.3.0');
    await userEvent.click(screen.getByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('You’re up to date: 0.3.0 is the latest version.')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Update and restart' })).not.toBeInTheDocument();
  });

  it('offers a newer version with its notes, then installs it showing progress', async () => {
    native.check = async () => newer;
    let report: (p: InstallProgress) => void = () => undefined;
    let finish: () => void = () => undefined;
    native.install = (onProgress) => {
      report = onProgress;
      return new Promise<void>((resolve) => {
        finish = resolve;
      });
    };
    renderWithProviders(<UpdatesSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('Version 0.4.0 is available')).toBeInTheDocument();
    expect(screen.getByText(/Faster reports\./)).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Update and restart' }));
    expect(await screen.findByText('Downloading…')).toBeInTheDocument();
    report({ event: 'downloading', downloaded: 512, total: 2048 });
    expect(await screen.findByText('Downloading… 25%')).toBeInTheDocument();
    report({ event: 'installing' });
    expect(await screen.findByText(/Installing…/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Check for updates' })).toBeDisabled();
    finish();
  });

  it('explains a failed check or install in words the user can act on', async () => {
    native.check = () => Promise.reject('Couldn’t check for updates. Check your internet connection and try again.');
    renderWithProviders(<UpdatesSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText(/Check your internet connection/)).toBeInTheDocument();

    native.check = async () => newer;
    let attempts = 0;
    native.install = () => {
      attempts += 1;
      return Promise.reject('Installing needs your computer’s password. The update wasn’t installed because the password was cancelled.');
    };
    await userEvent.click(screen.getByRole('button', { name: 'Check for updates' }));
    await userEvent.click(await screen.findByRole('button', { name: 'Update and restart' }));
    expect(await screen.findByText('The update wasn’t installed')).toBeInTheDocument();
    expect(screen.getByText(/needs your computer’s password/)).toBeInTheDocument();
    // The update is still there to try again.
    await userEvent.click(screen.getByRole('button', { name: 'Update and restart' }));
    await waitFor(() => expect(attempts).toBe(2));
  });

  it('offers nothing to click when this build can’t update itself, or in a browser', async () => {
    native.support = { enabled: false, current_version: '0.3.0' };
    const { unmount } = renderWithProviders(<UpdatesSettings />);
    expect(await screen.findByText('This version doesn’t update itself')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Check for updates' })).not.toBeInTheDocument();
    unmount();
    native.desktop = false;
    renderWithProviders(<UpdatesSettings />);
    expect(screen.getByText('Only in the desktop app')).toBeInTheDocument();
  });
});

describe('the details', () => {
  it('shows a download without a known size, and an update without a date or notes', async () => {
    native.check = async () => ({ ...newer, published_at: null, notes: null });
    let report: (p: InstallProgress) => void = () => undefined;
    native.install = (onProgress) => {
      report = onProgress;
      return new Promise<void>(() => undefined);
    };
    renderWithProviders(<UpdatesSettings />);
    await userEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));
    expect(await screen.findByText('Version 0.4.0 is available')).toBeInTheDocument();
    expect(screen.queryByText(/Released/)).not.toBeInTheDocument();
    expect(screen.queryByText(/Faster reports/)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Update and restart' }));
    report({ event: 'downloading', downloaded: 4096, total: null });
    await waitFor(() => expect(screen.getByText('Downloading…')).toBeInTheDocument());
    report({ event: 'downloading', downloaded: 4096, total: 1024 });
    expect(await screen.findByText('Downloading… 100%')).toBeInTheDocument();
  });

  it('can try again when the app can’t say whether it updates itself', async () => {
    // The first try and its two retries (services/api/query.ts) fail.
    native.supportFailures = [1, 2, 3].map(() => new Error('The desktop app didn’t answer.'));
    renderWithProviders(<UpdatesSettings />);
    expect(await screen.findByText('Couldn’t load this', undefined, { timeout: 10_000 })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('button', { name: 'Check for updates' })).toBeInTheDocument();
  });

  it('shows an error state when the app gives no answer', async () => {
    native.support = null;
    renderWithProviders(<UpdatesSettings />);
    expect(await screen.findByText('Couldn’t load this')).toBeInTheDocument();
  });

  it('turns any failure into words', () => {
    expect(updateErrorMessage('Check for updates first.')).toBe('Check for updates first.');
    expect(updateErrorMessage(new Error('The download didn’t finish.'))).toBe('The download didn’t finish.');
    expect(updateErrorMessage('')).toBe('Something went wrong. Try again.');
    expect(updateErrorMessage(new Error(''))).toBe('Something went wrong. Try again.');
    expect(updateErrorMessage({ code: 1 })).toBe('Something went wrong. Try again.');
  });
});

describe('the settings menu', () => {
  it('lists Updates only in a desktop build that updates itself', async () => {
    const { unmount } = renderWithProviders(<SettingsLayout />, { path: '/settings/business' });
    expect(await screen.findByRole('link', { name: 'Updates' })).toBeInTheDocument();
    unmount();

    native.support = { enabled: false, current_version: '0.3.0' };
    renderWithProviders(<SettingsLayout />, { path: '/settings/business' });
    await waitFor(() => expect(screen.getByRole('link', { name: 'Your data' })).toBeInTheDocument());
    expect(screen.queryByRole('link', { name: 'Updates' })).not.toBeInTheDocument();
  });
});
