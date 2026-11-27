import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup, openFromSidebar } from './helpers';

/** A 1×1 PNG: the API checks a receipt's bytes, not just its name. */
const RECEIPT_PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
  'base64',
);

test('a customer added from the dashboard opens on its own page', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Customer Studio');

  await page.getByRole('link', { name: 'Add customer' }).first().click();
  const dialog = page.getByRole('dialog', { name: 'Add customer' });
  await dialog.getByRole('textbox', { name: 'Name', exact: true }).fill('Kākā Consulting');
  await dialog.getByRole('textbox', { name: 'Email' }).fill('accounts@kaka.example');
  await dialog.getByRole('button', { name: 'Add customer' }).click();

  await expect(page.getByRole('heading', { name: 'Kākā Consulting' })).toBeVisible();
  await openFromSidebar(page, 'Customers');
  await expect(page.getByRole('link', { name: 'Kākā Consulting' }).or(page.getByText('Kākā Consulting')).first()).toBeVisible();
});

test('an expense is recorded with a receipt and counts on the dashboard', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Expense Studio');

  await page.getByRole('link', { name: 'Add expense' }).first().click();
  const dialog = page.getByRole('dialog', { name: 'Add expense' });
  await dialog.getByRole('textbox', { name: 'Amount paid' }).fill('115');
  await dialog.getByRole('textbox', { name: 'Supplier' }).fill('Spark');
  await dialog.getByRole('combobox', { name: 'Category' }).click();
  await page.getByRole('option', { name: 'Telephone' }).click();
  await expect(dialog.getByText('How this will be recorded')).toBeVisible();
  await dialog.getByRole('button', { name: 'Add expense' }).click();

  const drawer = page.getByRole('dialog', { name: 'Spark' });
  await expect(drawer.getByText('How it’s recorded')).toBeVisible();
  await drawer.locator('input[type="file"]').setInputFiles({ name: 'spark-receipt.png', mimeType: 'image/png', buffer: RECEIPT_PNG });
  await expect(drawer.getByText('spark-receipt.png')).toBeVisible();
  await expect(drawer.getByText('Expense: Spark')).toBeVisible();
  await page.keyboard.press('Escape');

  await expect(page.getByRole('img', { name: '1 receipt attached' })).toBeVisible();
  await page.goto('/#/dashboard');
  await expect(page.getByText('$115.00').first()).toBeVisible();
});

test('income without an invoice is recorded and listed', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Income Studio');

  await page.getByRole('link', { name: 'Record income' }).first().click();
  const dialog = page.getByRole('dialog', { name: 'Record income' });
  await dialog.getByLabel('Amount received').fill('250');
  await dialog.getByLabel('Payer').fill('Market stall');
  await dialog.getByRole('button', { name: 'Record income' }).click();

  await expect(page.getByRole('cell', { name: 'Market stall' })).toBeVisible();
  await expect(page.getByText('$250.00').first()).toBeVisible();
});
