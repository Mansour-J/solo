import { screen, within } from '@testing-library/react';
import { renderWithProviders } from '@/test/render';
import { BRAND } from '@/branding/brand';
import { AboutSettings, LicenceSettings } from './AboutSettings';

describe('Settings › About', () => {
  it('names the author, the purpose and the code, and says the records stay on this computer', () => {
    renderWithProviders(<AboutSettings />, { path: '/settings/about' });
    expect(screen.getByText(BRAND.author)).toBeInTheDocument();
    expect(screen.getByText(/sole traders .* GST and income tax/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'View the code on GitHub' })).toBeInTheDocument();
    expect(screen.getByText('No analytics or tracking.')).toBeInTheDocument();
    expect(screen.getByText('Everything stays on your own machine.')).toBeInTheDocument();
    expect(screen.getByText('The code is available to inspect.')).toBeInTheDocument();
  });
});

describe('Settings › Licence', () => {
  it('shows what the licence allows with ticks and crosses, and links to the full text', () => {
    renderWithProviders(<LicenceSettings />, { path: '/settings/licence' });
    const rows = screen.getAllByRole('row').slice(1);
    const answer = (text: string) =>
      within(rows.find((r) => r.textContent?.includes(text)) as HTMLElement)
        .getByRole('img')
        .getAttribute('aria-label');
    expect(answer('Use it for your business')).toBe('Yes');
    expect(answer('Read and inspect the code')).toBe('Yes');
    expect(answer('closed source')).toBe('No');
    expect(answer('Expect a warranty')).toBe('No');
    expect(screen.getByRole('link', { name: 'read the full licence' })).toHaveAttribute('href', `${BRAND.repositoryUrl}/blob/main/LICENSE`);
  });
});
