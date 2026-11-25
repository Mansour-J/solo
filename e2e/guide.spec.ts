import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

test('the guide walks through the app and opens the screen each step describes', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Kererū Contracting');

  // From the dashboard's getting-started card, and from the sidebar.
  await page.getByRole('link', { name: 'Step-by-step guide' }).click();
  await expect(page.getByRole('heading', { name: 'Guide', level: 1 })).toBeVisible();
  await expect(page.getByText('0 of 13 chapters done')).toBeVisible();

  // The contents jump to a chapter; a chapter can be marked as done, and it stays done.
  await page.getByRole('navigation', { name: 'Guide contents' }).getByRole('button', { name: 'Make your yearly report' }).click();
  const yearEnd = page.getByRole('region', { name: '12. Make your yearly report' });
  await expect(yearEnd).toBeInViewport();
  await yearEnd.getByRole('checkbox', { name: 'Mark “Make your yearly report” as done' }).check();
  await expect(page.getByText('1 of 13 chapters done')).toBeVisible();

  // A step's link opens the real form.
  await page.getByRole('region', { name: '7. Record what you spend' }).getByRole('link', { name: 'Add expense' }).click();
  await expect(page.getByRole('dialog', { name: 'Add expense' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog', { name: 'Add expense' })).toBeHidden();

  await page.getByRole('link', { name: 'Guide', exact: true }).click();
  await expect(page.getByRole('checkbox', { name: 'Mark “Make your yearly report” as done' })).toBeChecked();
});
