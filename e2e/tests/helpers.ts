import { expect, type Page } from '@playwright/test';
import { PASSWORD } from '../playwright.config';

/** Logs in through the login screen and waits for the dashboard. */
export async function login(page: Page, password = PASSWORD): Promise<void> {
  await page.goto('/login');
  await page.getByLabel('Password').fill(password);
  await page.getByRole('button', { name: 'Log in' }).click();
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
}

/** Adds `id` from the builder's protocol palette (search + click). */
export async function addProtocol(page: Page, id: string): Promise<void> {
  const palette = page.getByRole('region', { name: 'Protocols' });
  await palette.getByLabel('Search protocols').fill(id);
  await palette.getByRole('button', { name: new RegExp(`\\b${id}$`) }).first().click();
}

/** A name unique to this run. */
export function uniqueName(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}`;
}
