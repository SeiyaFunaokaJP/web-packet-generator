import { expect, test } from '@playwright/test';
import * as net from 'node:net';
import { login } from './helpers';

/**
 * A tiny Telnet server: asks for the window size (DO NAWS), prints a prompt and
 * echoes what it receives (CR as CR LF), minus Telnet commands.
 */
async function telnetServer(): Promise<{ port: number; received: Buffer[]; close: () => void }> {
  const received: Buffer[] = [];
  const server = net.createServer((sock) => {
    sock.write(Buffer.from([255, 251, 1, 255, 251, 3, 255, 253, 31]));
    sock.write('\x1b[1mlab-router>\x1b[0m ');
    sock.on('data', (d: Buffer) => {
      received.push(d);
      const out: number[] = [];
      for (let i = 0; i < d.length; i++) {
        const b = d[i];
        if (b === 255) {
          // IAC: skip the command (SB ... SE as a whole)
          if (d[i + 1] === 250) {
            while (i < d.length && !(d[i] === 255 && d[i + 1] === 240)) i++;
            i++;
          } else {
            i += 2;
          }
        } else if (b === 13) {
          out.push(13, 10);
          if (d[i + 1] === 0) i++;
        } else if (b >= 32 && b < 127) {
          out.push(b);
        }
      }
      if (out.length) sock.write(Buffer.from(out));
    });
    sock.on('error', () => {});
  });
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = (server.address() as net.AddressInfo).port;
  return { port, received, close: () => server.close() };
}

test('telnet session in the web terminal', async ({ page }) => {
  const srv = await telnetServer();
  try {
    await login(page);
    await page.getByRole('link', { name: 'Terminal' }).first().click();
    await expect(page.getByRole('heading', { level: 1, name: 'Terminal' })).toBeVisible();

    await page.getByRole('combobox', { name: 'Protocol' }).selectOption('telnet');
    await page.getByRole('textbox', { name: 'Host' }).fill('127.0.0.1');
    await page.getByRole('spinbutton', { name: 'Port' }).fill(String(srv.port));
    await page.getByRole('button', { name: 'Connect', exact: true }).click();

    const tab = page.getByRole('tab', { name: new RegExp(`127\\.0\\.0\\.1:${srv.port}`) });
    await expect(tab).toHaveAttribute('aria-selected', 'true');
    const screen = page.locator(`#term-host-${await tab.getAttribute('id').then((id) => id?.replace('term-tab-', ''))} .xterm-rows`);
    await expect(screen).toContainText('lab-router>', { timeout: 15_000 });
    await expect(page.getByRole('status').filter({ hasText: 'Connected to' })).toBeVisible();

    // Keystrokes reach the device and its echo comes back.
    await page.locator('.xterm-helper-textarea').first().focus();
    await page.keyboard.type('show version');
    await page.keyboard.press('Enter');
    await expect(screen).toContainText('show version');
    await expect
      .poll(() => Buffer.concat(srv.received).includes(Buffer.from('show version\r\0')))
      .toBe(true);

    // The session log is listed while recording.
    await expect(page.getByRole('region', { name: 'Session logs' }).getByText(/_telnet_127\.0\.0\.1_/)).toBeVisible();

    await page.getByRole('button', { name: 'Disconnect' }).click();
    await expect(page.getByRole('status').filter({ hasText: 'Disconnected' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Reconnect' })).toBeVisible();
  } finally {
    srv.close();
  }
});
