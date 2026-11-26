import { expect, test } from '@playwright/test';
import { createBusiness, startBusinessSetup } from './helpers';

// Backing up only: a restore replaces every business on the server, which the tests running alongside
// share. The local server's own test restores a backup (services/local/tests/local_server.rs).
test('backs up the books to a file and shows when', async ({ page }) => {
  await startBusinessSetup(page);
  await createBusiness(page, 'Backup Studio');

  await page.goto('/#/settings/data');
  await expect(page.getByRole('heading', { name: 'Back up and restore' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Restore from a backup' })).toBeVisible();

  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Back up now' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toMatch(/^solo-backup-\d{4}-\d{2}-\d{2}\.solobackup$/);
  await expect(page.getByText(/^Last backup:/)).toBeVisible();
});
