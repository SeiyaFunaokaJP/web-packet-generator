// Fail early with a clear message when the server was built without the web UI.
import { request } from '@playwright/test';
import config from './playwright.config';

export default async function globalSetup(): Promise<void> {
  const ctx = await request.newContext({ baseURL: config.use?.baseURL });
  const html = await (await ctx.get('/')).text();
  await ctx.dispose();
  if (html.includes('has not been built yet')) {
    throw new Error(
      'The server has no web UI: run `trunk build` in crates/wpg-web, then rebuild wpg-cli (see e2e/README.md).',
    );
  }
}
