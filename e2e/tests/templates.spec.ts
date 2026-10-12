import { expect, test } from '@playwright/test';
import { login } from './helpers';

test.beforeEach(async ({ page }) => {
  await login(page);
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Templates' }).click();
});

test('search narrows the bundled library', async ({ page }) => {
  const search = page.getByPlaceholder('Search name, description, tag, protocol…');
  await search.fill('vxlan');
  await expect(page.getByText(/vxlan/i).first()).toBeVisible();
  await search.fill('no-such-template-zzz');
  await expect(page.getByText('No template matches.')).toBeVisible();
});

test('a library template opens in the builder', async ({ page }) => {
  await page.getByPlaceholder('Search name, description, tag, protocol…').fill('arp-sweep');
  await page.getByRole('link', { name: 'Open in the builder' }).first().click();
  await expect(page).toHaveURL(/\/builder/);
  const layers = page.getByRole('region', { name: 'Layers' });
  await expect(layers.getByText('arp', { exact: true })).toBeVisible();
  await expect(page.getByTitle('The in-browser preview matches the server render.')).toBeVisible();
});
