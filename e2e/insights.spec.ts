import { expect, test, type Page } from '@playwright/test';
import { createBusiness, currentBusinessId, startBusinessSetup } from './helpers';

const headers = { 'x-client-kind': 'web' };

function shift(date: string, days: number): string {
  const d = new Date(`${date}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

/** Seeds an overdue $1,000 invoice through the API (issued 40 days ago, due 20 days ago). */
async function seedOverdueInvoice(page: Page) {
  const base = `/api/v1/businesses/${await currentBusinessId(page)}`;
  const { today } = (await (await page.request.get(base)).json()) as { today: string };
  const post = async (path: string, data: unknown) => {
    const response = await page.request.post(`${base}${path}`, { headers, data });
    expect(response.ok(), `${path}: ${await response.text()}`).toBeTruthy();
    return (await response.json()) as { id: string };
  };
  const customer = await post('/customers', { name: 'Tūī Media' });
  const invoice = await post('/invoices', {
    customer_id: customer.id,
    issue_date: shift(today, -40),
    due_date: shift(today, -20),
    lines: [{ description: 'Website', quantity: '1', unit_price: 100_000, tax_treatment: 'no_gst' }],
  });
  await post(`/invoices/${invoice.id}/issue`, {});
}

test('insights explain the figures, and a notice can be dismissed', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Insights Studio');
  await seedOverdueInvoice(page);

  // The dashboard points out what needs attention.
  await page.goto('/#/dashboard');
  await page.reload();
  await expect(page.getByText('Worth knowing')).toBeVisible();
  await expect(page.getByText('You have 1 overdue invoice ($1,000.00).')).toBeVisible();

  await page.getByRole('link', { name: 'Insights' }).first().click();
  await expect(page.getByText('Owed to you', { exact: true })).toBeVisible();
  await expect(page.getByText('Customers owe you $1,000.00 on 1 invoice, $1,000.00 of it overdue.')).toBeVisible();
  // Not enough history yet for comparisons: the section says why instead of guessing.
  await expect(page.getByText(/at least two complete months/)).toBeVisible();

  await page.getByRole('button', { name: 'Dismiss: You have 1 overdue invoice ($1,000.00).' }).click();
  await expect(page.getByText('You have 1 overdue invoice ($1,000.00).')).toBeHidden();
  await expect(page.getByRole('button', { name: 'Show 1 dismissed notice again' })).toBeVisible();

  await page.goto('/#/dashboard');
  await expect(page.getByRole('heading', { name: 'Income and expenses' })).toBeVisible();
  await expect(page.getByText('Worth knowing')).toBeHidden();
});
