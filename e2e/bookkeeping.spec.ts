import { expect, test, type Page } from '@playwright/test';
import { createBusiness, currentBusinessId, startBusinessSetup } from './helpers';

/** Creates a customer in this test's business through the API; returns its id. */
async function apiCustomer(page: Page, name: string): Promise<string> {
  const response = await page.request.post(`/api/v1/businesses/${await currentBusinessId(page)}/customers`, {
    headers: { 'x-client-kind': 'web' },
    data: { name, email: 'accounts@customer.example' },
  });
  expect(response.ok()).toBeTruthy();
  return ((await response.json()) as { id: string }).id;
}

async function chooseCustomer(page: Page, name: string) {
  await page.getByRole('combobox', { name: 'Customer' }).click();
  await page.getByRole('option', { name }).click();
}

async function fillFirstLine(page: Page, description: string, quantity: string, price: string) {
  await page.getByRole('textbox', { name: 'Description, line 1' }).fill(description);
  await page.getByRole('textbox', { name: 'Quantity, line 1' }).fill(quantity);
  await page.getByRole('textbox', { name: 'Price, line 1' }).fill(price);
}

test('an invoice goes from draft to paid and shows on the dashboard', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Invoice Studio');
  await apiCustomer(page, 'Kererū Café');

  await page.goto('/#/invoices/new');
  await chooseCustomer(page, 'Kererū Café');
  await fillFirstLine(page, 'Brand workshop', '4', '250');
  await expect(page.getByText('$1,000.00').last()).toBeVisible();
  await page.getByRole('button', { name: 'Save and issue' }).click();

  await expect(page.getByRole('heading', { name: /Invoice INV-0001/ })).toBeVisible();
  await page.getByRole('button', { name: 'Record payment' }).click();
  const dialog = page.getByRole('dialog', { name: 'Record payment' });
  await expect(dialog.getByLabel('Amount')).toHaveValue('1000.00');
  await dialog.getByRole('button', { name: 'Record payment' }).click();
  await expect(page.getByText('Payment recorded')).toBeVisible();
  await expect(page.getByText('Paid', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('In your books')).toBeVisible();

  await page.goto('/#/dashboard');
  await expect(page.getByRole('button', { name: /Income earned: view details/ }).or(page.getByText('Income earned'))).toBeVisible();
  await expect(page.getByText('$1,000.00').first()).toBeVisible();
});

test('an accepted quote becomes a draft invoice', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Quote Studio');
  await apiCustomer(page, 'Pūkeko Builders');

  await page.goto('/#/quotes/new');
  await chooseCustomer(page, 'Pūkeko Builders');
  await fillFirstLine(page, 'Deck design', '1', '1800');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name: /Quote QU-0001/ })).toBeVisible();

  await page.getByRole('button', { name: 'Customer’s answer' }).click();
  await page.getByRole('menuitem', { name: 'Accepted' }).click();
  await page.getByRole('button', { name: 'Create invoice' }).click();
  await expect(page.getByRole('heading', { name: /Draft invoice/ })).toBeVisible();
  await expect(page.getByText('Deck design')).toBeVisible();
});

test('invoice PDFs download', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'PDF Studio');
  await apiCustomer(page, 'Tōtara Trust');
  await page.goto('/#/invoices/new');
  await chooseCustomer(page, 'Tōtara Trust');
  await fillFirstLine(page, 'Consulting', '2', '150');
  await page.getByRole('button', { name: 'Save and issue' }).click();
  await expect(page.getByRole('heading', { name: /Invoice INV-0001/ })).toBeVisible();
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'PDF', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('Invoice INV-0001.pdf');
});
