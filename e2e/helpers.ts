import { expect, type Page } from '@playwright/test';

/**
 * Opens a new business's set-up. The app has one owner and no sign-in, and every test shares that
 * owner, so each test works in a business of its own: onboarding selects the business it creates.
 */
export async function startBusinessSetup(page: Page) {
  await page.goto('/#/onboarding');
  await expect(page.getByText('Tell us a little about your business')).toBeVisible();
}

/** Completes onboarding with the minimum: a business name. */
export async function createBusiness(page: Page, name: string) {
  await page.getByLabel('Business name').fill(name);
  for (let step = 0; step < 3; step++) await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByRole('button', { name: 'Create business' }).click();
  await expect(page.getByText('Your business is ready')).toBeVisible();
  await page.getByRole('button', { name: 'Go to dashboard' }).click();
  await expect(page.getByText('Finish setting up')).toBeVisible();
}

/** The business this test created (onboarding selects it), for seeding records through the API. */
export async function currentBusinessId(page: Page): Promise<string> {
  const id = await page.evaluate(() => window.localStorage.getItem('selected-business'));
  if (!id) throw new Error('No business selected: create one with createBusiness first');
  return id;
}

/** The sidebar group each module sits in (`NAV_TREE` in apps/client/src/app/nav.ts). */
const SIDEBAR_GROUPS: Record<string, string> = {
  Invoices: 'Sales',
  Quotes: 'Sales',
  Customers: 'Sales',
  Income: 'Sales',
  Expenses: 'Spending',
  Mileage: 'Spending',
  GST: 'Tax',
  'Income tax': 'Tax',
  'Year end': 'Tax',
  Reports: 'Planning',
  Insights: 'Planning',
  Allocations: 'Planning',
};

/** Opens a module from the sidebar, opening its group first when it is closed. */
export async function openFromSidebar(page: Page, name: string) {
  const sidebar = page.getByRole('navigation', { name: 'Main' });
  const link = sidebar.getByRole('link', { name, exact: true });
  const group = SIDEBAR_GROUPS[name];
  if (group && !(await link.isVisible())) await sidebar.getByRole('button', { name: group, exact: true }).click();
  await link.click();
}
