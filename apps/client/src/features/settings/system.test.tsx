import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { renderWithProviders } from '@/test/render';
import { EmailSettingsPage } from './EmailSettings';
import { NoticesEmailCard } from './NoticesEmail';
import { SystemStatusPage } from './SystemStatus';

afterEach(() => vi.restoreAllMocks());

const notSetUp = {
  server_managed: false,
  configured: false,
  enabled: false,
  host: '',
  port: 587,
  security: 'starttls',
  username: '',
  has_password: false,
  from_name: '',
  from_address: '',
  tested_at: null,
  test_error: null,
};

describe('email settings', () => {
  it('fills in a provider and saves the account', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      if (request.method === 'PUT') {
        writes.push(await request.json());
        return Response.json({ ...notSetUp, configured: true, enabled: true, host: 'smtp.gmail.com', from_address: 'me@example.com' });
      }
      return Response.json(notSetUp);
    });
    renderWithProviders(<EmailSettingsPage />, { path: '/settings/email' });
    await userEvent.click(await screen.findByRole('combobox', { name: 'Fill in for' }));
    await userEvent.click(await screen.findByRole('option', { name: 'Gmail', hidden: true }));
    await userEvent.type(screen.getByLabelText(/^User name/), 'me@example.com');
    await userEvent.type(screen.getByLabelText(/^Password/), 'app-password');
    await userEvent.type(screen.getByLabelText(/Send from/), 'me@example.com');
    await userEvent.click(screen.getByRole('button', { name: 'Save email settings' }));
    await waitFor(() => expect(writes).toHaveLength(1));
    expect(writes[0]).toMatchObject({
      host: 'smtp.gmail.com',
      port: 587,
      security: 'starttls',
      username: 'me@example.com',
      password: 'app-password',
      from_address: 'me@example.com',
      enabled: true,
    });
  });

  it('shows why the last test failed', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      Response.json({
        ...notSetUp,
        configured: true,
        enabled: true,
        host: 'smtp.example.com',
        from_address: 'me@example.com',
        tested_at: '2026-11-25T00:00:00Z',
        test_error: '535 authentication failed',
      }),
    );
    renderWithProviders(<EmailSettingsPage />, { path: '/settings/email' });
    expect(await screen.findByText('The last test failed')).toBeInTheDocument();
    expect(screen.getByText('535 authentication failed')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send test email' })).toBeInTheDocument();
  });
});

describe('system status', () => {
  it('lists each check with what to do', async () => {
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const path = new URL((input as Request).url).pathname;
      if (path.endsWith('/meta')) return Response.json({ brand: { name: 'Solo' }, email_enabled: false, version: '0.3.0', currencies: [] });
      return Response.json({
        version: '0.3.0',
        schema_version: 27,
        checks: [
          {
            key: 'database',
            name: 'Database',
            status: 'ok',
            detail: 'Your books are stored and answering.',
            action: null,
            link: null,
            took_ms: 2,
          },
          {
            key: 'email',
            name: 'Email',
            status: 'warning',
            detail: 'Not set up.',
            action: 'Add your email account.',
            link: '/settings/email',
            took_ms: 1,
          },
        ],
      });
    });
    renderWithProviders(<SystemStatusPage />, { path: '/settings/status' });
    expect(await screen.findByText('Your books are stored and answering.')).toBeInTheDocument();
    expect(screen.getByText('Working')).toBeInTheDocument();
    expect(screen.getByText('Check this')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Open settings' })).toHaveAttribute('href', '/settings/email');
  });
});

describe('weekly notices email', () => {
  it('turns it on with the default address', async () => {
    const writes: unknown[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const request = input as Request;
      if (request.method === 'PUT') writes.push(await request.json());
      return Response.json({ enabled: false, to: null, default_to: 'me@example.com', last_sent_at: null });
    });
    renderWithProviders(<NoticesEmailCard />);
    expect(await screen.findByText('Leave empty to use me@example.com.')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('switch', { name: 'Email me my notices each week' }));
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() => expect(writes).toEqual([{ enabled: true, to: null }]));
  });
});
