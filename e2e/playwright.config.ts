// End-to-end tests of the web UI against a real `wpg serve` (ROADMAP M4).
//
// Prerequisites (see README.md): `trunk build` in crates/wpg-web, then
// `cargo build -p wpg-cli` so the server embeds the freshly built UI.
// `WPG_BIN` overrides the server binary, `WPG_E2E_PORT` the port.
import { defineConfig, devices } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { PASSWORD } from './specs/helpers';

const root = path.resolve(__dirname, '..');
const port = Number(process.env.WPG_E2E_PORT ?? 18765);
const bin = process.env.WPG_BIN ?? path.join(root, 'target', 'debug', process.platform === 'win32' ? 'wpg.exe' : 'wpg');
const dataDir = path.join(__dirname, '.data');

// Start every run from an empty data directory (user templates, job history). The
// config is also loaded in the worker processes, which must not touch it.
if (!process.env.TEST_WORKER_INDEX) {
  fs.rmSync(dataDir, { recursive: true, force: true });
}

export default defineConfig({
  testDir: './specs',
  // One server with shared state (templates, jobs): run the specs one after another.
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  globalSetup: './global-setup.ts',
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    viewport: { width: 1400, height: 1000 },
    locale: 'en-US',
    trace: 'retain-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1400, height: 1000 } } }],
  webServer: {
    command: [
      JSON.stringify(bin),
      'serve',
      `--bind 127.0.0.1:${port}`,
      `--data-dir ${JSON.stringify(dataDir)}`,
      `--password ${PASSWORD}`,
      `--library-path ${JSON.stringify(path.join(root, 'templates'))}`,
    ].join(' '),
    url: `http://127.0.0.1:${port}/api/auth/status`,
    reuseExistingServer: false,
    timeout: 60_000,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
