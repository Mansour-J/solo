import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

test('opens without signing in and sets up a business', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Sign in' })).toHaveCount(0);

  await startBusinessSetup(page);
  await page.getByLabel('Business name').fill('Kōwhai Design');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByLabel('Business email').fill('hello@kowhai.example');
  await page.getByLabel('Street address').fill('12 Cuba Street');
  await page.getByLabel('Town or city').fill('Wellington');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByRole('switch', { name: 'I’m registered for GST' }).check();
  await page.getByLabel('GST number').fill('49-091-850');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByRole('button', { name: 'Create business' }).click();
  await page.getByRole('button', { name: 'Go to dashboard' }).click();

  await expect(page.getByRole('heading', { name: /^Kia ora/ })).toBeVisible();
  await expect(page.getByText('Registered · 49-091-850')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Your menu' })).toBeVisible();
  await page.getByRole('button', { name: 'Your menu' }).click();
  await expect(page.getByRole('menuitem', { name: 'Sign out' })).toHaveCount(0);
});

test('business settings and invoice defaults are saved', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Settings Studio');

  await page.goto('/#/settings/invoicing');
  await page.getByLabel('Invoice prefix').fill('SS-');
  await page.getByLabel('Account number').fill('12-3456-7890123-00');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page.getByText('Settings saved')).toBeVisible();
  await expect(page.getByText('Next invoice: SS-0001')).toBeVisible();

  await page.goto('/#/settings/activity');
  await expect(page.getByText('Invoice and expense settings updated')).toBeVisible();

  await page.goto('/#/dashboard');
  await expect(page.getByText('1 of 3 done')).toBeVisible();
});

test('the settings have no sign-in, password or device pages', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Plain Settings Ltd');
  await page.goto('/#/settings/profile');
  await expect(page.getByLabel('First name')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Security' })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Your data' })).toBeVisible();
});
