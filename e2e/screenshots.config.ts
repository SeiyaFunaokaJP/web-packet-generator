// Documentation screenshots and the builder GIF (ROADMAP M11), taken from a real
// `wpg serve` with the same server setup as the end-to-end tests.
//
//   npx playwright test -c screenshots.config.ts      # or: just screenshots
//
// The output goes to docs/{en,ja}/src/images/ (see screenshots/capture.spec.ts);
// `npx playwright test` with the default config does not run it.
import { defineConfig } from '@playwright/test';
import base from './playwright.config';

export default defineConfig({
  ...base,
  testDir: './screenshots',
  retries: 0,
  reporter: 'list',
  // The GIF frames are written here before ffmpeg assembles them.
  outputDir: './test-results/screenshots',
  use: { ...base.use, trace: 'off' },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }],
});
