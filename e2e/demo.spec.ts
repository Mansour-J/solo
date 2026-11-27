import { expect, test } from '@playwright/test';

// The demo business (SL-710). There's one demo per owner, and the suite shares one owner, so this
// journey starts from a reset (which builds a demo whether or not there is one) and removes it at the
// end. A true first start (no businesses) is covered by the component tests.
test('explores the demo business, then resets and removes it', async ({ page }) => {
  await page.goto('/#/welcome?demo=reset');
  await page.getByRole('button', { name: 'Reset the demo' }).click();

  const banner = page.getByLabel('Demo business');
  await expect(banner).toBeVisible({ timeout: 60_000 });
  await expect(page.getByRole('button', { name: /Current business: Fernbird Consulting \(demo\)/ })).toBeVisible();

  await page.goto('/#/invoices');
  await expect(page.getByText('Harbour Talent Partners (demo)').first()).toBeVisible();

  // It’s marked as the demo on the server too.
  const businesses = await page.request.get('/api/v1/businesses', { headers: { 'x-client-kind': 'web' } });
  const demo = ((await businesses.json()) as { id: string; is_demo: boolean }[]).find((b) => b.is_demo);
  expect(demo).toBeTruthy();

  await banner.getByRole('button', { name: 'Remove demo' }).click();
  await page.getByRole('button', { name: 'Remove the demo' }).click();
  await expect(page).not.toHaveURL(/demo=remove/);
  await expect(page.getByLabel('Demo business')).toHaveCount(0);
  const after = await page.request.get('/api/v1/businesses', { headers: { 'x-client-kind': 'web' } });
  expect(((await after.json()) as { is_demo: boolean }[]).some((b) => b.is_demo)).toBe(false);
});
