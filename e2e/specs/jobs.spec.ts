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

  // Closed, it is listed under History.
  await detail.getByRole('button', { name: 'Close' }).click();
  await page.getByRole('tab', { name: 'History' }).click();
  await expect(page.getByRole('main').getByText('e2e-null-job')).toBeVisible();
});
