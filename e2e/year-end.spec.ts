import { expect, test, type Page } from '@playwright/test';
import { createBusiness, currentBusinessId, startBusinessSetup, openFromSidebar } from './helpers';

const headers = { 'x-client-kind': 'web' };

/** Seeds a $5,000 issued invoice (no GST) dated today; returns its ID. */
async function seedInvoice(page: Page): Promise<string> {
  const base = `/api/v1/businesses/${await currentBusinessId(page)}`;
  const { today } = (await (await page.request.get(base)).json()) as { today: string };
  const post = async (path: string, data: unknown) => {
    const response = await page.request.post(`${base}${path}`, { headers, data });
    expect(response.ok(), `${path}: ${await response.text()}`).toBeTruthy();
    return (await response.json()) as { id: string };
  };
  const customer = await post('/customers', { name: 'Contract client' });
  const invoice = await post('/invoices', {
    customer_id: customer.id,
    issue_date: today,
    due_date: today,
    lines: [{ description: 'Contract work', quantity: '1', unit_price: 500_000, tax_treatment: 'no_gst' }],
  });
  await post(`/invoices/${invoice.id}/issue`, {});
  return invoice.id;
}

test('withholding tax, drawings and depreciation reach the balance sheet', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Year End Studio');
  const invoice = await seedInvoice(page);

  // The customer withheld $1,000 of tax from the $5,000 they paid.
  await page.goto(`/#/invoices/${invoice}`);
  await page.getByRole('button', { name: 'Record payment' }).click();
  const payment = page.getByRole('dialog', { name: 'Record payment' });
  await payment.getByLabel('Tax withheld').fill('1000');
  await payment.getByRole('button', { name: 'Record payment' }).click();
  await expect(page.getByText('Payment recorded')).toBeVisible();

  // A $300 drawing.
  await openFromSidebar(page, 'Year end');
  await expect(page.getByText('Nothing recorded yet')).toBeVisible();
  await page.getByRole('button', { name: 'Record money in or out' }).first().click();
  const movement = page.getByRole('dialog', { name: 'Record money in or out' });
  await movement.getByLabel('Amount').fill('300');
  await movement.getByLabel('Description').fill('Personal transfer');
  await movement.getByRole('button', { name: 'Record' }).click();
  await expect(page.getByText('Personal transfer')).toBeVisible();

  // A $2,400 laptop bought today at 50% diminishing value.
  await page.getByRole('tab', { name: 'Assets' }).click();
  await page.getByRole('button', { name: 'Add asset' }).first().click();
  const asset = page.getByRole('dialog', { name: 'Add asset' });
  await asset.getByLabel('Name').fill('Laptop');
  await asset.getByLabel('Cost excluding GST').fill('2400');
  await asset.getByLabel(/^Rate/).fill('50');
  await asset.getByRole('button', { name: 'Add asset' }).click();
  await expect(page.getByRole('cell', { name: 'Laptop' }).or(page.getByText('Laptop')).first()).toBeVisible();

  // This year's depreciation: from the month bought to the year end.
  await page.getByRole('combobox', { name: 'Financial year ending' }).click();
  await page.getByRole('option').first().click();
  await page.getByRole('button', { name: 'Post depreciation' }).click();
  await expect(page.getByText(/Depreciation for the year ending .* posted/)).toBeVisible();
  // Bought in November: five months to 31 March, 2,400 × 50% × 5/12.
  await expect(page.getByRole('row', { name: /^Laptop/ }).last()).toContainText('$500.00');

  // The balance sheet balances: equity = earnings − drawings − tax withheld.
  await openFromSidebar(page, 'Reports');
  await page.getByRole('navigation', { name: 'Reports' }).getByText('Balance sheet').click();
  await expect(page.getByRole('row', { name: /^Business bank account/ })).toContainText('$1,300.00');
  await expect(page.getByRole('row', { name: /^Drawings/ })).toContainText('-$300.00');
  await expect(page.getByRole('row', { name: /^Withholding tax paid/ })).toContainText('-$1,000.00');
  const net = await page.getByRole('row', { name: /^Net assets/ }).innerText();
  const equity = await page.getByRole('row', { name: /^Total equity/ }).innerText();
  expect(net.split(/\s+/)[2]).toBe(equity.split(/\s+/)[2]);
});
