import { expect, test } from '@playwright/test';
import { login } from './helpers';

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
