import { expect, test } from '@playwright/test';
import { login, uniqueName } from './helpers';

test('create and run a job on the null backend', async ({ page }) => {
  await login(page);
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Jobs' }).click();
  await page.getByRole('main').getByRole('link', { name: 'New job' }).click();

  const dialog = page.getByRole('dialog', { name: 'New job' });
  await dialog.getByLabel('Template').selectOption({ index: 1 });
  await dialog.getByLabel('Backend').selectOption('null');
  await dialog.getByLabel('Rate').fill('1000pps');
  await expect(dialog.getByText('→ 1.00 kpps')).toBeVisible();
  await dialog.getByLabel('Packet count').fill('200');
  const name = uniqueName('e2e-job');
  await dialog.getByLabel('Name', { exact: true }).fill(name);
  await dialog.getByRole('button', { name: 'Create and start' }).click();

  // The null backend never touches an interface: no confirmation, and the job's
  // detail drawer opens on the run, which ends when the packet count is reached.
  await expect(dialog).toBeHidden();
  const detail = page.getByRole('dialog', { name });
  await expect(detail.getByText('Packet count reached').first()).toBeVisible({ timeout: 30_000 });
  const stats = detail.getByRole('region', { name: 'Live statistics' });
  await expect(stats.getByRole('definition').filter({ hasText: /^200$/ })).toBeVisible();

  await detail.getByRole('button', { name: 'Close' }).click();
  const row = page.getByRole('row', { name: new RegExp(name) });
  await expect(row.getByRole('cell', { name: 'Finished' })).toBeVisible();
});
