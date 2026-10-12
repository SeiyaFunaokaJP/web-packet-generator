import { expect, test } from '@playwright/test';
import { addLayers, login, unique } from './helpers';

test.beforeEach(async ({ page }) => {
  await login(page);
  await page.goto('/builder');
});

test('builds Ethernet / IPv4 / UDP and matches the server render', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);

  const hex = page.getByRole('region', { name: 'Hex' });
  // 14 + 20 + 8 octets, padded to the 60-octet minimum; EtherType 0x0800, then
  // IPv4 version 4 / IHL 5 (the hex cells are separate elements, so no spaces).
  await expect(hex).toContainText('60 B');
  await expect(hex).toContainText('080045');

  const tree = page.getByRole('region', { name: 'Tree' });
  await expect(tree.getByRole('button', { name: /^total_length = 28 auto/ })).toBeVisible();
  await expect(tree.getByRole('button', { name: /^protocol = 17 auto/ })).toBeVisible();

  // The in-browser (WASM) build is compared with POST /api/render.
  await expect(page.getByText('matches', { exact: true })).toBeVisible();
});

test('generator, stream estimate and send time', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^2 IPv4/ }).click();

  const fields = page.getByRole('region', { name: 'Field editor' });
  const src = fields.getByRole('group', { name: 'src' });
  await src.getByRole('button', { name: 'Gen' }).click();
  await src.getByRole('textbox').first().fill('10.0.0.1-10.0.0.250');

  await expect(fields).toContainText('expands to 250');
  await expect(page.getByRole('group', { name: 'Stream index' })).toContainText('0 / 250');

  await expect(page.getByText('send time: set a rate')).toBeVisible();
  await page.getByRole('textbox', { name: 'Rate' }).fill('1000pps');
  await expect(page.getByText('~250 ms to send')).toBeVisible();
  await page.getByRole('textbox', { name: 'Rate' }).fill('10pps');
  await expect(page.getByText('~25.0 s to send')).toBeVisible();
  await page.getByRole('textbox', { name: 'Rate' }).fill('bogus');
  await expect(page.getByRole('textbox', { name: 'Rate' })).toHaveAttribute('aria-invalid', 'true');

  // Stepping through the stream changes the source address.
  await page.getByRole('button', { name: 'Next packet' }).click();
  await expect(page.getByRole('region', { name: 'Tree' }).getByRole('button', { name: /^src = 10\.0\.0\.2 / })).toBeVisible();
});

test('undo and redo layer edits', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4']);
  const layers = page.getByRole('region', { name: 'Layers' }).getByRole('listitem');
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(layers).toHaveCount(1);
  await page.getByRole('button', { name: 'Redo' }).click();
  await expect(layers).toHaveCount(2);
  await page.keyboard.press('Control+z');
  await expect(layers).toHaveCount(1);
});

test('save a template and find it in the library', async ({ page }) => {
  const name = unique('e2e-udp');
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  await page.getByRole('textbox', { name: 'Template name' }).fill(name);
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Template saved.' })).toBeVisible();

  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Templates' }).click();
  await page.getByRole('searchbox').fill(name);
  await expect(page.getByRole('main').getByText(name, { exact: true })).toBeVisible();
});

test('open a library template in the builder', async ({ page }) => {
  await page.goto('/builder?template=protocols-l2-arp-request');
  await expect(page.getByRole('textbox', { name: 'Template name' })).toHaveValue('arp-request');
  await expect(page.getByRole('region', { name: 'Layers' }).getByRole('listitem')).toHaveCount(2);
  await expect(page.getByRole('region', { name: 'Tree' }).getByRole('button', { name: /^ARP/ })).toBeVisible();
});
