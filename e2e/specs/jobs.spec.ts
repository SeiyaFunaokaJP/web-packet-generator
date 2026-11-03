import { expect, test } from '@playwright/test';
import { login } from './helpers';

test('create a job on the null backend and see it finish', async ({ page }) => {
  await login(page);
  await page.goto('/jobs?new=protocols-l4-udp');

  const dialog = page.getByRole('dialog', { name: 'New job' });
  await expect(dialog.getByRole('combobox', { name: 'Template' })).toHaveValue('protocols-l4-udp');
  await dialog.getByRole('combobox', { name: 'Backend' }).selectOption('null');
  await dialog.getByRole('textbox', { name: 'Rate' }).fill('1000pps');
  await dialog.getByRole('textbox', { name: 'Packet count' }).fill('200');
  await dialog.getByRole('textbox', { name: 'Name', exact: true }).fill('e2e-null-job');
  await dialog.getByRole('button', { name: 'Create and start' }).click();
  await expect(dialog).toBeHidden();

  // 200 packets at 1000 pps: the detail drawer opens and the job finishes in ~0.2 s.
  const detail = page.getByRole('dialog', { name: 'e2e-null-job' });
  await expect(detail).toContainText('Packet count reached', { timeout: 15_000 });
  await expect(detail.getByRole('region', { name: 'Per stream' }).getByRole('row', { name: /^1 protocols-l4-udp 1 200 / })).toBeVisible();

  // A finished job is not a dead end: its template links to the builder, and
  // "Edit as new job" opens the form with the same settings.
  await expect(detail.getByRole('link', { name: 'protocols-l4-udp' })).toHaveAttribute('href', '/builder?template=protocols-l4-udp');
  await expect(detail.getByRole('button', { name: 'Run again' })).toBeVisible();
  await detail.getByRole('link', { name: 'Edit as new job' }).click();
  const copy = page.getByRole('dialog', { name: 'New job' });
  await expect(copy.getByRole('combobox', { name: 'Template' })).toHaveValue('protocols-l4-udp');
  await expect(copy.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('e2e-null-job');
  await expect(copy.getByRole('textbox', { name: 'Packet count' })).toHaveValue('200');
  // Unsaved input: Escape asks before closing.
  await copy.getByRole('textbox', { name: 'Rate' }).fill('2000pps');
  await page.keyboard.press('Escape');
  const ask = page.getByRole('alertdialog', { name: 'Discard changes?' });
  await ask.getByRole('button', { name: 'Keep editing' }).click();
  await expect(copy.getByRole('textbox', { name: 'Rate' })).toHaveValue('2000pps');
  await page.keyboard.press('Escape');
  await ask.getByRole('button', { name: 'Discard' }).click();
  await expect(copy).toBeHidden();

  // It is listed under History.
  await page.goto('/jobs');
  await page.getByRole('tab', { name: 'History' }).click();
  await expect(page.getByRole('main').getByText('e2e-null-job')).toBeVisible();
});
