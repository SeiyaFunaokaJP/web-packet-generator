import { expect, test } from '@playwright/test';
import { login } from './helpers';

test('switch the UI to Japanese and back', async ({ page }) => {
  await login(page);
  const language = page.locator('header select');
  await language.selectOption('ja');
  const nav = page.getByRole('navigation').first();
  await expect(nav.getByRole('link', { name: 'ビルダー' })).toBeVisible();

  // The choice survives a reload (localStorage).
  await page.reload();
  await expect(page.getByRole('navigation').first().getByRole('link', { name: 'ビルダー' })).toBeVisible();
  await expect(language).toHaveValue('ja');

  await language.selectOption('en');
  await expect(page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Builder' })).toBeVisible();
});

test('every language of the menu translates the navigation', async ({ page }) => {
  await login(page);
  const language = page.locator('header select');
  const tags = await language.locator('option').evaluateAll((os) => os.map((o) => (o as HTMLOptionElement).value));
  expect(tags).toEqual(['ja', 'en', 'zh-CN', 'zh-TW', 'ko', 'de', 'fr', 'es']);
  const builder = page.getByRole('navigation').first().getByRole('link').nth(1);
  const labels = new Set<string>();
  for (const tag of tags) {
    await language.selectOption(tag);
    await expect(page.locator('html')).toHaveAttribute('lang', tag);
    labels.add((await builder.innerText()).trim());
  }
  // The languages do not all share one word for "Builder".
  expect(labels.size).toBeGreaterThan(4);
  await language.selectOption('en');
});
