import { readFile } from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

test('a vehicle, a rate and a trip produce a mileage claim in the report, the dashboard and the CSV', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Mileage Studio');

  // First visit: set up the vehicle with a rate of our own.
  await page.getByRole('link', { name: 'Mileage' }).first().click();
  await page.getByRole('button', { name: 'Add vehicle' }).click();
  const vehicleDialog = page.getByRole('dialog', { name: 'Add vehicle' });
  await vehicleDialog.getByRole('textbox', { name: 'Name', exact: true }).fill('Corolla');
  await vehicleDialog.getByRole('textbox', { name: 'Registration' }).fill('abc123');
  await vehicleDialog.getByRole('textbox', { name: 'Rate per km' }).fill('1.04');
  await vehicleDialog.getByRole('button', { name: 'Add vehicle' }).click();
  await expect(page.getByRole('button', { name: 'Record trip' })).toBeVisible();

  // A 42.5 km trip from the odometer, all for the business: 42.5 × $1.04 = $44.20.
  await page.getByRole('button', { name: 'Record trip' }).click();
  const tripDialog = page.getByRole('dialog', { name: 'Record a trip' });
  await tripDialog.getByRole('textbox', { name: 'Purpose' }).fill('Client workshop');
  await tripDialog.getByRole('textbox', { name: 'To' }).fill('Kererū Café');
  await tripDialog.getByText('Odometer readings').click();
  await tripDialog.getByRole('textbox', { name: 'Start reading' }).fill('10000');
  await tripDialog.getByRole('textbox', { name: 'End reading' }).fill('10042.5');
  await expect(tripDialog.getByText('$44.20')).toBeVisible();
  await tripDialog.getByRole('button', { name: 'Record trip' }).click();

  await expect(page.getByText('Trip recorded')).toBeVisible();
  await expect(page.getByRole('cell', { name: /Client workshop/ })).toBeVisible();
  await expect(page.getByText('$44.20').first()).toBeVisible();

  await page.getByRole('tab', { name: 'Report' }).click();
  await expect(page.getByText('Mileage claim')).toBeVisible();
  await expect(page.getByRole('cell', { name: 'Corolla' })).toBeVisible();

  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export CSV' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/^Mileage \d{4}-\d{2}-\d{2} to \d{4}-\d{2}-\d{2}\.csv$/);
  const csv = await readFile(await file.path(), 'utf8');
  expect(csv).toContain('Client workshop');
  expect(csv).toContain('44.20');

  // The claim is a business expense on the dashboard.
  await page.goto('/#/dashboard');
  await expect(page.getByText('Mileage claims')).toBeVisible();
  await expect(page.getByRole('link', { name: 'View report' })).toBeVisible();
});
