import { expect, test } from '@playwright/test';
import { login, PASSWORD } from './helpers';

test('unauthenticated visits go to the login screen', async ({ page }) => {
  await page.goto('/jobs');
  await expect(page).toHaveURL(/\/login/);
  await expect(page.getByRole('heading', { name: 'Log in' })).toBeVisible();
});

test('a wrong password is rejected', async ({ page }) => {
  await page.goto('/login');
  await page.locator('#login-password').fill('not-the-password');
  await page.getByRole('button', { name: 'Log in' }).click();
  await expect(page.getByRole('alert')).toHaveText('Wrong password.');
  await expect(page).toHaveURL(/\/login/);
});

test('log in and out', async ({ page }) => {
  await login(page);
  await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await page.getByRole('button', { name: 'Log out' }).click();
  await expect(page).toHaveURL(/\/login/);
  await page.goto('/');
  await expect(page).toHaveURL(/\/login/);
});

test('a deep link keeps its query string through the login', async ({ page }) => {
  await page.goto('/jobs?new=protocols-l4-udp');
  await expect(page).toHaveURL(/\/login\?next=%2Fjobs%3Fnew%3Dprotocols-l4-udp$/);
  await page.locator('#login-password').fill(PASSWORD);
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/\/jobs\?new=protocols-l4-udp$/);
  const dialog = page.getByRole('dialog', { name: 'New job' });
  await expect(dialog.getByRole('combobox', { name: 'Template' })).toHaveValue('protocols-l4-udp');
});
