import { expect, test, type Page } from '@playwright/test';
import { createBusiness, currentBusinessId, startBusinessSetup, openFromSidebar } from './helpers';

const headers = { 'x-client-kind': 'web' };

/** Seeds through the API: a customer with a $1,000 invoice part paid ($400), and $230 of income, all today. */
async function seed(page: Page): Promise<string> {
  const base = `/api/v1/businesses/${await currentBusinessId(page)}`;
  // The business's today, in its time zone.
  const { today } = (await (await page.request.get(base)).json()) as { today: string };
  const post = async (path: string, data: unknown) => {
    const response = await page.request.post(`${base}${path}`, { headers, data });
    expect(response.ok(), `${path}: ${await response.text()}`).toBeTruthy();
    return (await response.json()) as { id: string };
  };
  const customer = await post('/customers', { name: 'Kererū Café' });
  const invoice = await post('/invoices', {
    customer_id: customer.id,
    issue_date: today,
    due_date: today,
    lines: [{ description: 'Brand workshop', quantity: '1', unit_price: 100_000, tax_treatment: 'no_gst' }],
  });
  await post(`/invoices/${invoice.id}/issue`, {});
  await post(`/invoices/${invoice.id}/payments`, { payment_date: today, amount: 40_000, method: 'bank_transfer' });
  await post('/income', {
    income_date: today,
    payer: 'Market stall',
    income_type: 'sales',
    gross_amount: 23_000,
    gst_treatment: 'no_gst',
    payment_method: 'cash',
  });
  return customer.id;
}

test('reports show figures from the records, export as PDF and give a customer their statement', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Reports Studio');
  const customer = await seed(page);

  // Profit and loss for this financial year (accrual): the invoice and the income are sales.
  await openFromSidebar(page, 'Reports');
  await expect(page.getByText('Profit and loss', { exact: true }).first()).toBeVisible();
  const income = page.getByRole('row', { name: /^Sales/ }).first();
  await expect(income).toContainText('$1,230.00');
  // On the cash basis only what was received counts: $400 + $230.
  await page.getByLabel('Basis').getByText('Cash', { exact: true }).click();
  await expect(page.getByRole('row', { name: /^Sales/ }).first()).toContainText('$630.00');

  // What's still owed, exported as a PDF.
  await page.getByRole('navigation', { name: 'Reports' }).getByText('Accounts receivable').click();
  await expect(page.getByRole('row', { name: /Kererū Café/ }).first()).toContainText('$600.00');
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export' }).click();
  await page.getByRole('menuitem', { name: 'PDF', exact: true }).click();
  expect((await download).suggestedFilename()).toMatch(/^Accounts receivable \d{4}-\d{2}-\d{2}\.pdf$/);

  // The customer's statement, from their page.
  await page.goto(`/#/customers/${customer}`);
  await page.getByRole('link', { name: 'Statement' }).click();
  await expect(page.getByText('For Kererū Café')).toBeVisible();
  await expect(page.getByRole('row', { name: /Amount owing/ })).toContainText('$600.00');
});
