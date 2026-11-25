import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

test('a rule proposes a split for income, the user adjusts and confirms it, and deleting the income reverses it', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Allocation Studio');

  // First visit: nothing is planned until there's a rule.
  await page.getByRole('link', { name: 'Allocations' }).first().click();
  await expect(page.getByText('Plan where your money goes')).toBeVisible();
  await page.getByRole('button', { name: 'Add your first rule' }).click();
  const ruleDialog = page.getByRole('dialog', { name: 'Add rule' });
  await ruleDialog.getByRole('combobox', { name: 'Destination' }).click();
  await page.getByRole('option', { name: 'Income tax' }).click();
  await ruleDialog.getByRole('textbox', { name: 'Percentage' }).fill('20');
  await ruleDialog.getByRole('button', { name: 'Add rule' }).click();
  await expect(page.getByText('20% of the amount received → Income tax')).toBeVisible();
  // The preview splits an amount without recording anything.
  await expect(page.getByText('Try your rules')).toBeVisible();

  // Record $1,000 of income: the rule proposes $200 for income tax.
  await page.goto('/#/income');
  await page.getByRole('button', { name: 'Record income' }).first().click();
  const incomeDialog = page.getByRole('dialog', { name: 'Record income' });
  await incomeDialog.getByLabel('Amount received').fill('1000');
  await incomeDialog.getByLabel('Payer').fill('Market stall');
  await incomeDialog.getByRole('button', { name: 'Record income' }).click();
  await page.getByRole('link', { name: 'Review the allocation' }).click();

  const review = page.getByRole('dialog', { name: 'Allocation' });
  await expect(review.getByText('Market stall')).toBeVisible();
  const tax = review.getByRole('textbox', { name: 'Amount for Income tax' });
  await expect(tax).toHaveValue('200.00');
  await expect(review.getByText('Solo doesn’t move money', { exact: false })).toBeVisible();
  await tax.fill('250');
  await expect(review.getByText('Proposed $200.00')).toBeVisible();
  await expect(review.getByText('$750.00')).toBeVisible();
  await review.getByRole('button', { name: 'Confirm allocation' }).click();
  await expect(page.getByText('Allocation recorded')).toBeVisible();

  await expect(page.getByText('$250.00 set aside in total')).toBeVisible();
  await page.getByRole('tab', { name: 'History' }).click();
  await expect(page.getByRole('cell', { name: /Market stall/ })).toBeVisible();
  await expect(page.locator('tbody').getByText('Allocated', { exact: true })).toBeVisible();

  // The dashboard shows the balance.
  await page.goto('/#/dashboard');
  await expect(page.getByText('$250.00 set aside in total')).toBeVisible();

  // Deleting the income reverses the allocation, and the history keeps both.
  await page.goto('/#/income');
  await page.getByRole('row', { name: 'Edit income from Market stall' }).click();
  await page
    .getByRole('dialog', { name: /income/i })
    .getByRole('button', { name: 'Delete' })
    .click();
  await page.getByRole('button', { name: 'Delete income' }).click();
  await expect(page.getByText('Income deleted')).toBeVisible();
  await page.goto('/#/allocations?tab=history');
  await expect(page.locator('tbody').getByText('Reversed', { exact: true })).toBeVisible();
  await page.getByRole('tab', { name: 'Overview' }).click();
  await expect(page.getByText('$0.00 set aside in total')).toBeVisible();
});
