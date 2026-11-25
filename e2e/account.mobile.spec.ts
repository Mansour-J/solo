import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

test('phone layout: onboarding, navigation drawer and settings', async ({ page }) => {
  await startBusinessSetup(page);
  await expect(page.getByText('Step 1 of 5')).toBeVisible();
  await createBusiness(page, 'Harbour Consulting');

  await page.getByRole('button', { name: 'Toggle navigation' }).click();
  await page.getByRole('link', { name: 'Settings' }).click();
  await expect(page.getByRole('combobox', { name: 'Settings section' })).toHaveValue('Business details');
  await expect(page.getByLabel('Business name')).toHaveValue('Harbour Consulting');
});
