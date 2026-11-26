import { expect, test } from '@playwright/test';
import { createBusiness, currentBusinessId, startBusinessSetup } from './helpers';

const headers = { 'x-client-kind': 'web' };

test('business health points to what needs fixing and opens the screen that fixes it', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Health Studio');

  // An expense without a receipt.
  const base = `/api/v1/businesses/${await currentBusinessId(page)}`;
  const { today } = (await (await page.request.get(base)).json()) as { today: string };
  const categories = (await (await page.request.get(`${base}/expense-categories`, { headers })).json()) as { id: string }[];
  const expense = await page.request.post(`${base}/expenses`, {
    headers,
    data: {
      expense_date: today,
      supplier: 'Paper Test Supplies',
      category_id: categories[0]!.id,
      gross_amount: 2_000,
      gst_treatment: 'no_gst',
      payment_method: 'business_account',
      classification: 'business',
    },
  });
  expect(expense.ok(), await expense.text()).toBeTruthy();

  await page.goto('/#/settings/health');
  await expect(page.getByRole('heading', { name: 'A few things to look at' })).toBeVisible();
  await expect(page.getByText('Your invoices have no bank account or payment instructions.')).toBeVisible();
  await expect(page.getByText('1 expense this financial year has no receipt.')).toBeVisible();
  await expect(page.getByRole('button', { name: /^Doesn’t apply/ })).toBeVisible();

  await page.getByRole('link', { name: 'Attach receipts' }).click();
  await expect(page).toHaveURL(/#\/expenses\?receipt=missing/);
  await expect(page.getByRole('combobox', { name: 'Receipt' })).toHaveValue('Missing receipt');
  await expect(page.getByText('Paper Test Supplies').first()).toBeVisible();

  await page.goto('/#/settings/health');
  await page.getByRole('link', { name: 'Add payment details' }).click();
  await expect(page).toHaveURL(/#\/settings\/invoicing$/);
});
