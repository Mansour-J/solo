import type { Business } from '@solo/api-client';
import type * as BusinessesApi from '@/features/businesses/api';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { matchRoutes } from 'react-router';
import { FOOTER_NAV_ITEMS, NAV_ITEMS } from '@/app/nav';
import { appRoutes } from '@/app/router';
import { settingsRoutes } from '@/features/settings/routes';
import { YEAR_END_TABS } from '@/features/yearend/forms';
import { renderWithProviders } from '@/test/render';
import { emphasis, GLOSSARY, GUIDE_CHAPTERS } from './content';
import { GuidePage } from './GuidePage';

const { business } = vi.hoisted(() => ({
  business: {
    id: 'b1',
    currency: 'NZD',
    role: 'owner',
    today: '2026-11-21',
    gst_registered: true,
    reporting_basis: 'accrual',
    current_financial_year: { start: '2026-04-01', end: '2027-03-31', label: 'FY2027' },
  } as unknown as Business,
}));
vi.mock('@/features/businesses/api', async (importOriginal) => ({
  ...(await importOriginal<typeof BusinessesApi>()),
  useCurrentBusiness: () => business,
}));

afterEach(() => {
  localStorage.clear();
});

/** The client's source files as text, for checking the guide's labels against the screens. */
const SOURCES = import.meta.glob<string>(['/src/**/*.{ts,tsx}'], { query: '?raw', import: 'default', eager: true });

describe('guide content', () => {
  it('links every step to a real screen', () => {
    const links = GUIDE_CHAPTERS.flatMap((c) => c.steps.flatMap((s) => (s.link ? [s.link.to] : [])));
    expect(links.length).toBeGreaterThan(20);
    for (const to of links) {
      const url = new URL(to, 'https://app.test');
      const matches = matchRoutes(appRoutes, url.pathname);
      const route = matches?.at(-1)?.route;
      expect(route, `${to} has no route`).toBeDefined();
      expect(route?.path, `${to} only reaches the not-found page`).not.toBe('*');
    }
  });

  it('covers every screen in the navigation and settings', () => {
    const linked = GUIDE_CHAPTERS.flatMap((c) => c.steps.flatMap((s) => (s.link ? [new URL(s.link.to, 'https://app.test')] : [])));
    const paths = new Set(linked.map((u) => u.pathname));
    const screens = [...NAV_ITEMS, ...FOOTER_NAV_ITEMS]
      .map((i) => i.path)
      // The guide itself, and Settings (its pages are checked one by one below).
      .filter((p) => p !== '/guide' && p !== '/settings');
    const settings = (settingsRoutes[0]?.children ?? []).flatMap((r) => (r.path ? [`/settings/${r.path}`] : []));
    for (const path of [...screens, ...settings]) expect(paths, `no guide step links to ${path}`).toContain(path);
    // Every Year end tab has its own step.
    const tabs = new Set(linked.filter((u) => u.pathname === '/year-end').map((u) => u.searchParams.get('tab')));
    for (const tab of YEAR_END_TABS) expect(tabs, `no guide step links to Year end › ${tab}`).toContain(tab);
  });

  it('uses only labels that the app shows', () => {
    // Every **label** in the guide must appear in the app's own source, so renaming a button, field or
    // tab without updating the guide fails here. "Settings › Business details" is checked part by part.
    const source = Object.entries(SOURCES)
      .filter(([path]) => !path.endsWith('/guide/content.ts') && !/\.test\.tsx?$/.test(path))
      .map(([, text]) => text)
      .join('\n');
    const copy = GUIDE_CHAPTERS.flatMap((c) => [c.summary, ...c.steps.flatMap((s) => [s.body, s.tip ?? ''])]).join('\n');
    const labels = [...copy.matchAll(/\*\*([^*]+)\*\*/g)].flatMap((m) => m[1]!.split(' › '));
    expect(labels.length).toBeGreaterThan(100);
    const missing = [...new Set(labels)].filter((label) => !source.includes(label) && !source.includes(label.replace(/’/g, '\\u2019')));
    expect(missing, 'guide labels not found in the app').toEqual([]);
  });

  it('ends with the yearly report and its exports', () => {
    const yearEnd = GUIDE_CHAPTERS.find((c) => c.id === 'year-end');
    expect(yearEnd).toBeDefined();
    const text = yearEnd!.steps.map((s) => s.body).join(' ');
    for (const words of [
      '**Post depreciation**',
      '**Profit and loss**',
      '**Balance sheet**',
      '**Statement of changes in equity**',
      '**Export**',
    ]) {
      expect(text).toContain(words);
    }
  });

  it('marks the app’s own words in bold', () => {
    expect(emphasis('Choose **Add expense**, then **Save**.')).toEqual([
      { text: 'Choose ', strong: false },
      { text: 'Add expense', strong: true },
      { text: ', then ', strong: false },
      { text: 'Save', strong: true },
      { text: '.', strong: false },
    ]);
    expect(emphasis('Plain text')).toEqual([{ text: 'Plain text', strong: false }]);
  });

  it('uses typographic apostrophes in the copy', () => {
    const copy = [
      ...GUIDE_CHAPTERS.flatMap((c) => [
        c.title,
        c.summary,
        c.when ?? '',
        ...c.steps.flatMap((s) => [s.title, s.body, s.tip ?? '', s.link?.label ?? '']),
      ]),
      ...GLOSSARY.flatMap((g) => [g.term, g.meaning]),
    ];
    for (const text of copy) expect(text, text).not.toContain("'");
  });
});

describe('GuidePage', () => {
  it('shows every chapter in order, with contents and the glossary', () => {
    renderWithProviders(<GuidePage />);
    const chapters = GUIDE_CHAPTERS;
    const headings = screen.getAllByRole('heading', { level: 2 }).map((h) => h.textContent);
    expect(headings).toEqual([...chapters.map((c, i) => `${i + 1}. ${c.title}`), 'Words you’ll meet']);
    const contents = screen.getByRole('navigation', { name: 'Guide contents' });
    expect(within(contents).getAllByRole('button')).toHaveLength(chapters.length + 1);
    expect(screen.getByText('0 of 13 chapters done')).toBeInTheDocument();
    expect(screen.queryByText('Choose where your books are')).not.toBeInTheDocument();
  });

  it('links steps to the screens they describe', () => {
    renderWithProviders(<GuidePage />);
    const expenses = screen.getByRole('region', { name: '7. Record what you spend' });
    expect(within(expenses).getByRole('link', { name: 'Add expense' })).toHaveAttribute('href', '/expenses?new=1');
  });

  it('remembers chapters marked as done', async () => {
    const user = userEvent.setup();
    const { unmount } = renderWithProviders(<GuidePage />);
    await user.click(screen.getByRole('checkbox', { name: 'Mark “Before you start” as done' }));
    expect(screen.getByText('1 of 13 chapters done')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Continue: Set up your business/ })).toBeInTheDocument();
    expect(JSON.parse(localStorage.getItem('guide-done:b1') ?? '[]')).toEqual(['start']);

    unmount();
    renderWithProviders(<GuidePage />);
    expect(screen.getByRole('checkbox', { name: 'Mark “Before you start” as done' })).toBeChecked();
    await user.click(screen.getByRole('checkbox', { name: 'Mark “Before you start” as done' }));
    expect(screen.getByText('0 of 13 chapters done')).toBeInTheDocument();
  });
});
