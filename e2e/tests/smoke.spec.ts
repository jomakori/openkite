import { test, expect } from '@playwright/test';

/**
 * Smoke: app shell renders and routes work.
 *
 * Deliberately avoids cluster-dependent views (they render error/empty
 * states without a kubeconfig). Asserts the stable shell chrome only:
 * brand, sidebar nav, content outlet.
 */
test('app shell renders brand + primary nav', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'OpenKite' })).toBeVisible();
  await expect(page.locator('nav.nav')).toBeVisible();
  for (const label of ['Cluster', 'Workloads', 'Logs', 'Terminal', 'Config']) {
    await expect(page.locator('nav.nav').getByText(label, { exact: true }).first()).toBeVisible();
  }
});

test('unknown route renders 404 with back-home link', async ({ page }) => {
  await page.goto('/definitely-not-a-route');
  await expect(page.getByText('404', { exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Back home' })).toBeVisible();
});
