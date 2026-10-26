// Documentation screenshots (PNG) and the builder walkthrough (GIF), ROADMAP M11.
//
// Each screen is set up once in the English UI, shot, then the language is
// switched in place (the UI re-renders without losing state) and shot again:
// docs/en/src/images/<name>.png and docs/ja/src/images/<name>.png. The GIF is
// assembled by ffmpeg from one such shot per step (no video recording, so both
// languages come from the same run and the result is reproducible).
import { expect, test, type Locator, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { addLayers, login, unique } from '../specs/helpers';

const root = path.resolve(__dirname, '..', '..');
const imagesDir = (lang: 'en' | 'ja') => path.join(root, 'docs', lang, 'src', 'images');
const viewport = { width: 1280, height: 860 };
// Toasts ("Job … created.") are hidden in the shots.
const hideToasts = 'div.fixed[aria-live="polite"] { display: none !important; }';

test.use({ viewport, deviceScaleFactor: 1, colorScheme: 'light' });

/** Switches the UI language with the header menu, also behind an open dialog. */
async function setLanguage(page: Page, lang: 'en' | 'ja'): Promise<void> {
  await page.locator('header select').evaluate((el, l) => {
    const select = el as HTMLSelectElement;
    select.value = l;
    select.dispatchEvent(new Event('change', { bubbles: true }));
  }, lang);
  const builder = lang === 'ja' ? 'ビルダー' : 'Builder';
  await expect(page.getByRole('navigation').first().getByRole('link', { name: builder })).toBeVisible();
}

/** Scrolls everything inside `region` back to the top (clicks scroll the palette). */
async function scrollToTop(region: Locator): Promise<void> {
  await region.evaluate((el) => el.querySelectorAll('*').forEach((e) => (e.scrollTop = 0)));
}

/** Shoots the page in English, then in Japanese, and goes back to English. */
async function shoot(page: Page, name: string): Promise<void> {
  // Let transitions, fonts and live counters settle.
  await page.waitForTimeout(400);
  for (const lang of ['en', 'ja'] as const) {
    await setLanguage(page, lang);
    await page.waitForTimeout(200);
    await page.screenshot({ path: path.join(imagesDir(lang), `${name}.png`), style: hideToasts });
  }
  await setLanguage(page, 'en');
}

test('screens', async ({ page }) => {
  test.setTimeout(120_000);

  // The login screen has no header: use the saved preference.
  for (const lang of ['en', 'ja'] as const) {
    await page.goto('/login');
    await page.evaluate((l) => localStorage.setItem('wpg.locale', l), lang);
    await page.reload();
    await expect(page.locator('#login-password')).toBeVisible();
    await page.waitForTimeout(300);
    await page.screenshot({ path: path.join(imagesDir(lang), 'login.png') });
  }
  await page.evaluate(() => localStorage.setItem('wpg.locale', 'en'));

  await login(page);
  await shoot(page, 'dashboard');

  // Template library, searched for UDP templates.
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Templates' }).click();
  await page.getByRole('searchbox').fill('udp');
  await page.getByRole('main').getByRole('listitem').first().scrollIntoViewIfNeeded();
  await shoot(page, 'templates');

  // Builder: Ethernet / IPv4 / UDP with a source-address range.
  await page.goto('/builder');
  await addLayers(page, ['ethernet2', 'ipv4', 'udp', 'payload']);
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^2 IPv4/ }).click();
  const src = page.getByRole('region', { name: 'Field editor' }).getByRole('group', { name: 'src' });
  await src.getByRole('button', { name: 'Gen' }).click();
  await src.getByRole('textbox').first().fill('10.0.0.1-10.0.0.250');
  await page.getByRole('textbox', { name: 'Rate' }).fill('1000pps');
  await expect(page.getByText('matches', { exact: true })).toBeVisible();
  // The palette scrolled down to `payload`: show its top.
  await scrollToTop(page.getByRole('region', { name: 'Protocols' }));
  // Taller, so the hex dump and the tree are in the shot too.
  await page.setViewportSize({ width: viewport.width, height: 1240 });
  await shoot(page, 'builder');
  await page.setViewportSize(viewport);

  // Jobs: a signed dry run (its file is replayed on the receive page below).
  const name = unique('docs-udp');
  const file = `${name}.pcapng`;
  await page.goto('/jobs?new=protocols-l4-udp');
  const form = page.getByRole('dialog', { name: 'New job' });
  await expect(form.getByRole('combobox', { name: 'Template' })).toHaveValue('protocols-l4-udp');
  await form.getByRole('textbox', { name: 'Dry run: pcapng file name' }).fill(file);
  await form.getByRole('textbox', { name: 'Rate' }).fill('500pps');
  await form.getByRole('textbox', { name: 'Packet count' }).fill('1000');
  await form.getByRole('checkbox', { name: 'Stamp a verification signature' }).check();
  await form.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
  await shoot(page, 'job-new');
  await form.getByRole('button', { name: 'Create and start' }).click();
  const job = page.getByRole('dialog', { name });
  await expect(job).toContainText('Packet count reached', { timeout: 30_000 });
  await shoot(page, 'job-detail');
  await job.getByRole('button', { name: 'Close' }).click();

  // Receive: replay the dry run with signature verification.
  await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Receive' }).click();
  const start = page.getByRole('region', { name: 'New capture' });
  await start.getByRole('combobox', { name: 'Backend' }).selectOption('pcapfile');
  await start.getByRole('textbox', { name: 'Capture file' }).fill(file);
  await start.getByRole('textbox', { name: 'Filter' }).fill('udp and wpg');
  await start.getByRole('textbox', { name: 'Name', exact: true }).fill(name);
  await start.getByRole('button', { name: 'Start capture' }).click();
  const detail = page.getByRole('region', { name, exact: true });
  await expect(detail).toContainText('End of the capture file', { timeout: 30_000 });
  const frames = detail.getByRole('region', { name: 'Captured frames' });
  await frames.getByRole('button', { name: 'Frame 0' }).click();
  // Start the shot at the counters; scrollIntoView also scrolls the document, undo that.
  await detail.getByRole('region', { name: 'Counters' }).evaluate((el) => {
    el.scrollIntoView({ block: 'start' });
    window.scrollTo(0, 0);
  });
  await shoot(page, 'receive');
});

test('builder walkthrough GIF', async ({ page }) => {
  test.setTimeout(120_000);
  try {
    execFileSync('ffmpeg', ['-version'], { stdio: 'ignore' });
  } catch {
    test.skip(true, 'ffmpeg is not installed: the GIF is not regenerated');
  }

  // One frame per step in both languages, each shown for `seconds`.
  const work = test.info().outputPath('gif');
  const frames: { file: string; seconds: number }[] = [];
  const frame = async (seconds: number) => {
    await page.waitForTimeout(300);
    const file = `${String(frames.length).padStart(3, '0')}.png`;
    for (const lang of ['en', 'ja'] as const) {
      await setLanguage(page, lang);
      await page.waitForTimeout(150);
      await page.screenshot({ path: path.join(work, lang, file), style: hideToasts });
    }
    await setLanguage(page, 'en');
    frames.push({ file, seconds });
  };

  await login(page);
  await page.goto('/builder');
  await frame(1.5);
  for (const id of ['ethernet2', 'ipv4', 'udp', 'payload']) {
    const palette = page.getByRole('region', { name: 'Protocols' });
    await palette.getByRole('button', { name: new RegExp(` ${id}$`) }).click();
    await scrollToTop(palette);
    await frame(1);
  }
  await page.getByRole('region', { name: 'Layers' }).getByRole('button', { name: /^2 IPv4/ }).click();
  await frame(1.5);
  const src = page.getByRole('region', { name: 'Field editor' }).getByRole('group', { name: 'src' });
  await src.getByRole('button', { name: 'Gen' }).click();
  await frame(1);
  await src.getByRole('textbox').first().fill('10.0.0.1-10.0.0.250');
  await expect(page.getByRole('group', { name: 'Stream index' })).toContainText('0 / 250');
  await frame(2.5);
  for (let i = 0; i < 3; i++) {
    await page.getByRole('button', { name: 'Next packet' }).click();
    await frame(1);
  }
  frames[frames.length - 1].seconds = 3;

  for (const lang of ['en', 'ja'] as const) {
    const dir = path.join(work, lang);
    // ffmpeg concat list: the last file is repeated so its duration is honoured.
    const list = frames.map((f) => `file '${f.file}'\nduration ${f.seconds}\n`).join('') + `file '${frames[frames.length - 1].file}'\n`;
    fs.writeFileSync(path.join(dir, 'frames.txt'), list);
    fs.mkdirSync(imagesDir(lang), { recursive: true });
    execFileSync('ffmpeg', [
      '-y', '-loglevel', 'error', '-f', 'concat', '-safe', '0', '-i', path.join(dir, 'frames.txt'),
      '-vf', 'scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle',
      '-loop', '0', path.join(imagesDir(lang), 'builder.gif'),
    ]);
  }
});
