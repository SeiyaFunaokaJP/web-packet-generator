// End-to-end tests of the web UI (TODO M4 "E2E テスト (Playwright)").
//
// `webServer` starts `wpg serve` (debug build) on a free loopback port with a fresh
// data directory, the bundled template library and a fixed admin password. The
// front-end must have been built first (`trunk build --release` in crates/wpg-web)
// so wpg-server can embed crates/wpg-web/dist; `just e2e` does both.
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { defineConfig, devices } from '@playwright/test';

export const PASSWORD = 'e2e-admin-password';
const PORT = Number(process.env.WPG_E2E_PORT ?? 18080);
const root = resolve(__dirname, '..');
const dataDir = process.env.WPG_E2E_DATA_DIR ?? mkdtempSync(join(tmpdir(), 'wpg-e2e-'));
// An empty config file keeps a developer's ./wpg.toml out of the run.
const configFile = join(dataDir, 'wpg.toml');
writeFileSync(configFile, '');

export default defineConfig({
  testDir: './tests',
  // One server, one data directory: tests that create templates / jobs must not race.
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    locale: 'en-US',
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'], viewport: { width: 1440, height: 900 } } }],
  webServer: {
    command: `cargo run -q -p wpg-cli -- serve --bind 127.0.0.1:${PORT} --data-dir "${dataDir}" --library-path templates`,
    cwd: root,
    env: { WPG_PASSWORD: PASSWORD, WPG_CONFIG: configFile, RUST_LOG: 'warn' },
    url: `http://127.0.0.1:${PORT}/api/health`,
    // The first run compiles wpg-cli.
    timeout: 900_000,
    reuseExistingServer: false,
    stdout: 'ignore',
    stderr: 'pipe',
  },
});
