import { expect, test } from '@playwright/test';
import { login } from './helpers';

test('switch the UI to Japanese and back', async ({ page }) => {
  await login(page);
  await page.getByRole('button', { name: 'JA' }).click();
  const nav = page.getByRole('navigation').first();
  await expect(nav.getByRole('link', { name: 'ビルダー' })).toBeVisible();

  // The choice survives a reload (localStorage).
  await page.reload();
  await expect(page.getByRole('navigation').first().getByRole('link', { name: 'ビルダー' })).toBeVisible();

  await page.getByRole('button', { name: 'EN' }).click();
  await expect(page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Builder' })).toBeVisible();
});
