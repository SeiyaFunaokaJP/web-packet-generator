import { expect, test } from '@playwright/test';
import { login } from './helpers';

test('switch the language to Japanese and back', async ({ page }) => {
  await login(page);
  const nav = page.getByRole('navigation');
  await page.getByRole('button', { name: 'JA', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'ダッシュボード' })).toBeVisible();
  await page.getByRole('button', { name: 'EN', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
  await expect(nav.first()).toBeVisible();
});

test('narrow screens are view-only', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 900 });
  await login(page);
  await page.goto('/templates');
  await expect(page.getByText('Narrow screen: view only. Editing is available on wider screens.')).toBeVisible();
});

test('? opens the keyboard shortcut help', async ({ page }) => {
  await login(page);
  await page.locator('body').press('Shift+?');
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toBeHidden();
});
