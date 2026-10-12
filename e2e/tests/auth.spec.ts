import { expect, test } from '@playwright/test';
import { PASSWORD } from '../playwright.config';
import { login } from './helpers';

test('unauthenticated visits go through the login screen and come back', async ({ page }) => {
  await page.goto('/builder');
  await expect(page).toHaveURL(/\/login\?next=%2Fbuilder$/);
  await expect(page.getByLabel('Password')).toBeFocused();
  await page.getByLabel('Password').fill(PASSWORD);
  await page.getByRole('button', { name: 'Log in' }).click();
  await expect(page).toHaveURL(/\/builder$/);
  await expect(page.getByRole('region', { name: 'Protocols' })).toBeVisible();
});

test('a wrong password is rejected', async ({ page }) => {
  await page.goto('/login');
  await page.getByLabel('Password').fill('not-the-password');
  await page.getByRole('button', { name: 'Log in' }).click();
  await expect(page.getByText('Wrong password.')).toBeVisible();
  await expect(page).toHaveURL(/\/login$/);
});

test('log in, then log out from the settings screen', async ({ page }) => {
  await login(page);
  const nav = page.getByRole('navigation', { name: 'Main navigation' });
  for (const name of ['Dashboard', 'Builder', 'Jobs', 'Templates', 'Settings']) {
    await expect(nav.getByRole('link', { name })).toBeVisible();
  }
  await nav.getByRole('link', { name: 'Settings' }).click();
  await page.getByRole('main').getByRole('button', { name: 'Log out' }).click();
  await expect(page).toHaveURL(/\/login(\?|$)/);
  // The session is gone: protected routes redirect again.
  await page.goto('/jobs');
  await expect(page).toHaveURL(/\/login\?next=%2Fjobs$/);
});
