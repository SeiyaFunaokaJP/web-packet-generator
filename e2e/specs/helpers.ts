import { expect, type Page } from '@playwright/test';

/** Admin password the server is started with (playwright.config.ts). */
export const PASSWORD = 'e2e-password-123';

/** Logs in through the login screen and waits for the dashboard. */
export async function login(page: Page): Promise<void> {
  await page.goto('/login');
  await page.locator('#login-password').fill(PASSWORD);
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { level: 1, name: 'Dashboard' })).toBeVisible();
}

/** Adds protocols to the builder's layer stack from the palette, outermost first. */
export async function addLayers(page: Page, ids: string[]): Promise<void> {
  for (const id of ids) {
    await page
      .getByRole('region', { name: 'Protocols' })
      .getByRole('button', { name: new RegExp(` ${id}$`) })
      .click();
  }
  await expect(page.getByRole('region', { name: 'Layers' }).getByRole('listitem')).toHaveCount(ids.length);
}

/** A name unique to this run (the data directory is wiped per run, retries are not). */
export function unique(prefix: string): string {
  return `${prefix}-${Date.now().toString(36)}`;
}
