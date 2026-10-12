import { expect, test } from '@playwright/test';
import { login, unique } from './helpers';

// Receive / verification screen (TODO M9): a job writes signed frames to a
// dry-run pcapng file, a `pcapfile` capture replays it with verification on.
test('verify a signed dry run on the receive page', async ({ page }) => {
  const name = unique('e2e-sig');
  const file = `${name}.pcapng`;
  await login(page);

  // 1. A dry-run job with signatures (stream id 7), 50 frames.
  await page.goto('/jobs?new=protocols-l4-udp');
  const form = page.getByRole('dialog', { name: 'New job' });
  await expect(form.getByRole('combobox', { name: 'Template' })).toHaveValue('protocols-l4-udp');
  await form.getByRole('textbox', { name: 'Dry run: pcapng file name' }).fill(file);
  await form.getByRole('textbox', { name: 'Packet count' }).fill('50');
  const streamId = form.getByRole('textbox', { name: 'Signature stream id (first stream)' });
  await expect(streamId).toBeDisabled();
  await form.getByRole('checkbox', { name: 'Stamp a verification signature' }).check();
  await streamId.fill('7');
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
  await form.getByRole('button', { name: 'Create and start' }).click();
  await expect(form).toBeHidden();
  const job = page.getByRole('dialog', { name });
  await expect(job).toContainText('Packet count reached', { timeout: 15_000 });
  await expect(job).toContainText('On (stream id 7 and up)');
  await job.getByRole('button', { name: 'Close' }).click();

  // 2. Replay the file with verification on.
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Receive' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Receive' })).toBeVisible();
  const start = page.getByRole('region', { name: 'New capture' });
  await start.getByRole('combobox', { name: 'Backend' }).selectOption('pcapfile');
  await start.getByRole('textbox', { name: 'Capture file' }).fill(file);
  await start.getByRole('textbox', { name: 'Filter' }).fill('udp and wpg');
  await expect(start.getByText('Valid: udp and wpg')).toBeVisible();
  await expect(start.getByRole('checkbox', { name: 'Verify signatures (loss, order, latency)' })).toBeChecked();
  await start.getByRole('textbox', { name: 'Save as pcapng (file name)' }).fill(`${name}-rx.pcapng`);
  await start.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
  await start.getByRole('button', { name: 'Start capture' }).click();

  // 3. The detail: finished at the end of the file, 50 matched, nothing lost.
  const detail = page.getByRole('region', { name, exact: true });
  await expect(detail).toContainText('End of the capture file', { timeout: 15_000 });
  const verification = detail.getByRole('region', { name: 'Verification' });
  await expect(verification.getByRole('row', { name: /^7 50 0 – 49 0 \(0\.0 %\) 0 0 / })).toBeVisible();
  await expect(detail.getByRole('region', { name: 'Counters' })).toContainText('Matched50');
  await expect(detail.getByRole('link', { name: 'Download pcapng' })).toHaveAttribute(
    'href',
    /\/api\/captures\/[0-9a-f]+\/pcapng$/,
  );

  // 4. The first frames, one of them decoded.
  const frames = detail.getByRole('region', { name: 'Captured frames' });
  await expect(frames.getByRole('button', { name: /^Frame \d+$/ })).toHaveCount(50);
  await expect(frames.getByRole('row', { name: /stream 7 · seq 0$/ })).toBeVisible();
  await frames.getByRole('button', { name: 'Frame 0' }).click();
  const frame = frames.getByRole('group', { name: 'Frame 0' });
  await expect(frame.getByRole('region', { name: 'Tree' })).toContainText('dst_port');
  await expect(frame.getByRole('region', { name: 'Hex' })).toContainText('0000');

  // 5. Listed, then deleted.
  const list = page.getByRole('region', { name: 'Captures' });
  await expect(list.getByRole('row', { name: new RegExp(`^${name} `) })).toContainText('Finished');
  await list.getByRole('button', { name: `Delete “${name}”` }).click();
  const confirm = page.getByRole('dialog', { name: 'Delete this capture?' });
  await confirm.getByRole('button', { name: 'Delete' }).click();
  await expect(list.getByRole('button', { name: `Delete “${name}”` })).toHaveCount(0);
  await expect(detail).toBeHidden();
});

test('capture filter is validated while typing', async ({ page }) => {
  await login(page);
  await page.goto('/rx');
  const start = page.getByRole('region', { name: 'New capture' });
  const filter = start.getByRole('textbox', { name: 'Filter' });
  await expect(start.getByText('Empty: every frame matches.')).toBeVisible();

  await filter.fill('udp port');
  await expect(filter).toHaveAttribute('aria-invalid', 'true');
  await expect(start.getByText(/\(at character \d+\)$/)).toBeVisible();

  // An invalid filter blocks the start.
  await start.getByRole('combobox', { name: 'Backend' }).selectOption('null');
  await start.getByRole('button', { name: 'Start capture' }).click();
  await expect(page.getByRole('region', { name: 'Captures' })).not.toContainText('null:');

  await filter.fill('udp port 53');
  await expect(filter).toHaveAttribute('aria-invalid', 'false');
  await expect(start.getByText('Valid: udp port 53')).toBeVisible();
});
