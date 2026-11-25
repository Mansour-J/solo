import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type * as BusinessesApi from '@/features/businesses/api';
import { renderWithProviders } from '@/test/render';
import { EmailStatementModal } from './StatementEmail';

vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => ({
    id: 'b1',
    name: 'Test Ltd',
    email: 'hello@test.example',
    currency: 'NZD',
    today: '2026-11-25',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31' },
  }),
}));

afterEach(() => vi.restoreAllMocks());

it('emails the statement for the financial year to date by default', async () => {
  const sent: unknown[] = [];
  vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
    const request = input as Request;
    if (request.method === 'POST') sent.push(await request.json());
    return Response.json([]);
  });
  renderWithProviders(
    <EmailStatementModal opened onClose={() => {}} customer={{ id: 'c1', name: 'Tūī Media', email: 'accounts@tui.example' }} />,
  );
  expect(await screen.findByDisplayValue('Statement from Test Ltd')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: 'Send statement' }));
  await waitFor(() => expect(sent).toHaveLength(1));
  expect(sent[0]).toMatchObject({ from: '2026-04-01', to: '2026-11-25', email: 'accounts@tui.example' });
});
