import { expect, test } from '@playwright/test';
import { addProtocol, login, uniqueName } from './helpers';

test.beforeEach(async ({ page }) => {
  await login(page);
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Builder' }).click();
});

test('build Ethernet / IPv4 / UDP and match the server render', async ({ page }) => {
  for (const id of ['ethernet2', 'ipv4', 'udp']) await addProtocol(page, id);

  const layers = page.getByRole('region', { name: 'Layers' });
  await expect(layers.getByText('ethernet2', { exact: true })).toBeVisible();
  await expect(layers.getByText('ipv4', { exact: true })).toBeVisible();
  await expect(layers.getByText('udp', { exact: true })).toBeVisible();

  // 14 + 20 + 8 = 42 bytes, padded to the 60-byte minimum frame.
  await expect(page.getByText('expands to 1 (~60 B)')).toBeVisible();
  // The in-browser WASM build equals POST /api/render byte for byte.
  await expect(page.getByTitle('The in-browser preview matches the server render.')).toBeVisible();
});

test('a generator expands the stream and the send time is estimated', async ({ page }) => {
  for (const id of ['ethernet2', 'ipv4', 'udp']) await addProtocol(page, id);
  // UDP is selected after being added: put a 1000-value range on the source port.
  const srcPort = page.getByRole('group', { name: 'src_port' });
  await srcPort.getByRole('button', { name: 'Gen' }).click();
  await expect(srcPort.getByRole('button', { name: 'Gen' })).toHaveAttribute('aria-pressed', 'true');
  await srcPort.getByRole('textbox').first().fill('1000-1999');
  await expect(page.getByText(/expands to 1,000 \(~60\.0 kB\)/)).toBeVisible();

  const rate = page.getByLabel('Rate');
  await expect(page.getByText('Set a rate to estimate the send time.')).toBeVisible();
  await rate.fill('100pps');
  await expect(page.getByText('≈ 10.0 s to send')).toBeVisible();
  await rate.fill('50%');
  await expect(page.getByText('Send time depends on the link speed.')).toBeVisible();
  await rate.fill('12 parsecs');
  await expect(rate).toHaveAttribute('aria-invalid', 'true');
});

test('undo / redo a layer and save the template', async ({ page }) => {
  await addProtocol(page, 'ethernet2');
  await addProtocol(page, 'arp');
  const layers = page.getByRole('region', { name: 'Layers' });
  await expect(layers.getByText('arp', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(layers.getByText('arp', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Redo' }).click();
  await expect(layers.getByText('arp', { exact: true })).toBeVisible();

  const name = uniqueName('e2e-arp');
  await page.getByRole('textbox', { name: 'Template name' }).fill(name);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('Template saved.')).toBeVisible();

  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Templates' }).click();
  await page.getByPlaceholder('Search name, description, tag, protocol…').fill(name);
  await expect(page.getByText(name).first()).toBeVisible();
});
