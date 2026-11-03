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

  // The in-browser (WASM) build is compared with POST /api/render; only a
  // difference shows a badge, the toolbar carries the outcome.
  await expect(page.locator('[data-parity="match"]')).toHaveCount(1);
  await expect(page.getByText('Preview differs from server')).toHaveCount(0);
});

test('generator, stream estimate and send time', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^2 IPv4/ }).click();

  const fields = page.getByRole('region', { name: 'Field editor' });
  const src = fields.getByRole('group', { name: 'src' });
  await src.getByRole('button', { name: 'Gen' }).click();
  await src.getByRole('textbox').first().fill('10.0.0.1-10.0.0.250');

  await expect(fields).toContainText('expands to 250');
  await expect(page.getByRole('group', { name: 'Stream index' })).toContainText('Packet 1 of 250');

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

test('fields are prefilled, quick stacks fill an empty builder', async ({ page }) => {
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: 'Ethernet II / IPv4 / UDP / Payload' }).click();
  await expect(page.getByRole('region', { name: 'Layers' }).getByRole('listitem')).toHaveCount(4);

  // Ethernet II is selected: its addresses are editable inputs holding the defaults.
  const fields = page.getByRole('region', { name: 'Field editor' });
  const dst = fields.getByRole('group', { name: 'dst' });
  await expect(dst.getByRole('textbox')).toHaveValue('ff:ff:ff:ff:ff:ff');
  await dst.getByRole('textbox').fill('00:11:22:33:44:55');
  await expect(page.getByRole('region', { name: 'Tree' }).getByRole('button', { name: /^dst = 00:11:22:33:44:55/ })).toBeVisible();
  await dst.getByRole('button', { name: 'Back to default' }).click();
  await expect(dst.getByRole('textbox')).toHaveValue('ff:ff:ff:ff:ff:ff');
});

test('strict by default; "Allow malformed packets" unlocks computed fields', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^2 IPv4/ }).click();
  const fields = page.getByRole('region', { name: 'Field editor' });
  const csum = fields.getByRole('group', { name: 'header_checksum' });
  await expect(csum.getByRole('button', { name: 'Override' })).toHaveCount(0);

  const irregular = page.getByRole('switch', { name: 'Allow malformed packets' });
  await irregular.click();
  await csum.getByRole('button', { name: 'Override' }).click();
  await csum.getByRole('textbox').fill('0xbeef');
  await expect(csum).toContainText('differs from the computed checksum');

  // Back in strict mode the same packet is an error and cannot be saved.
  await irregular.click();
  await expect(page.getByRole('status').filter({ hasText: '1 out of spec' })).toBeVisible();
  await page.getByRole('textbox', { name: 'Template name' }).fill(unique('e2e-bad-csum'));
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByText('1 out-of-spec values.')).toBeVisible();
  await csum.getByRole('button', { name: 'Back to computed' }).click();
  await expect(page.getByRole('status').filter({ hasText: 'out of spec' })).toHaveCount(0);
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

test('new job saves the template first, asking for a name', async ({ page }) => {
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  // Let the preview of the finished stack settle before saving.
  await expect(page.locator('[data-parity="match"]')).toHaveCount(1);
  await page.getByRole('button', { name: 'New job', exact: true }).click();
  const prompt = page.getByRole('dialog', { name: 'Name this template' });
  await prompt.getByRole('textbox').fill(unique('e2e-job'));
  await prompt.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'New job' })).toBeVisible();

  // The saved packet does not come back as the next new template.
  await page.goto('/builder');
  await expect(page.getByRole('region', { name: 'Layers' }).getByRole('listitem')).toHaveCount(0);
});

test('a restored draft can be discarded', async ({ page }) => {
  await page.goto('/builder?template=protocols-l2-arp-request');
  const layers = page.getByRole('region', { name: 'Layers' }).getByRole('listitem');
  await expect(layers).toHaveCount(2);
  await page.getByRole('region', { name: 'Protocols' }).getByRole('button', { name: / payload$/ }).click();
  await expect(layers).toHaveCount(3);

  await page.reload();
  await expect(page.getByText('Restored unsaved changes')).toBeVisible();
  await expect(layers).toHaveCount(3);
  await page.getByRole('button', { name: 'Discard', exact: true }).click();
  await expect(layers).toHaveCount(2);
  await expect(page.getByText('Restored unsaved changes')).toHaveCount(0);
});
