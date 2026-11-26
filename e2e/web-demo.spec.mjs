// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ブラウザのお試し版（US-14）。画面の操作だけで確かめる。page.request は使わない。

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test, expect } from '@playwright/test';

// 配信先と同じ CSP（試験用サーバーが付ける）で、違反が1件も出ないこと。AC-14.3 の「配信元以外へ出ない」の裏付け。
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    window.__cspViolations = [];
    document.addEventListener('securitypolicyviolation', (event) => {
      window.__cspViolations.push(`${event.violatedDirective} ${event.blockedURI}`);
    });
  });
  page.__cspConsole = [];
  // すべての試験で、配信元以外と /api/ への要求を数える（AC-14.3。開いてから操作し終えるまで）。
  page.__leaked = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.origin !== 'http://127.0.0.1:8770' || url.pathname.includes('/api/')) {
      page.__leaked.push(request.url());
    }
  });
  page.on('console', (message) => {
    if (/Content Security Policy/i.test(message.text())) page.__cspConsole.push(message.text());
  });
});

test.afterEach(async ({ page }) => {
  const seen = await page.evaluate(() => window.__cspViolations ?? []).catch(() => []);
  expect([...seen, ...page.__cspConsole]).toEqual([]);
  expect(page.__leaked).toEqual([]);
});

async function openDemo(page) {
  await page.goto('./');
  await expect(page.locator('main h1')).toHaveText('ホーム');
  await expect(page.locator('.sidebar-business')).toHaveText('サンプル事務所（デモ）');
}

async function go(page, hash) {
  await page.evaluate((next) => {
    if (location.hash === next) {
      window.dispatchEvent(new HashChangeEvent('hashchange'));
    } else {
      location.hash = next;
    }
  }, hash);
}

function textOf(body) {
  const start = body[0] === 0xef && body[1] === 0xbb && body[2] === 0xbf ? 3 : 0;
  return body.subarray(start).toString('utf8');
}

async function readDownload(page, click) {
  const pending = page.waitForEvent('download');
  await click();
  const download = await pending;
  const file = await download.path();
  return { name: download.suggestedFilename(), body: fs.readFileSync(file) };
}

test('AC-14.1 サブパスを開くだけで、架空の帳簿の画面が出る', async ({ page }) => {
  await openDemo(page);
  await expect(page.getByLabel('表示する月')).toHaveValue('2026-09');
  await expect(page.locator('.banner-title')).toHaveText('お試し版です');
  await expect(page.getByText('デモのデータです')).toHaveCount(0);
  const nums = await page.locator('main .num').allTextContents();
  expect(nums.some((text) => /[1-9]/.test(text))).toBe(true);
});

test('AC-14.2 登録すると仕訳帳と帳票に残り、再読み込みで最初の帳簿に戻る', async ({ page }) => {
  await openDemo(page);
  await go(page, '#/reports/income-statement?month=2026-09');
  await expect(page.locator('.report-title')).toContainText('2026年9月');
  const supplies = page.locator('.report-content tr', { hasText: '消耗品費' });
  await expect(supplies.first()).toBeVisible();
  const before = await supplies.first().innerText();

  await go(page, '#/entry');
  const form = page.locator('form.entry-form');
  await expect(form).toBeVisible();
  await expect(form.getByLabel('日付')).toHaveValue('2026-09-30');
  await form.getByLabel('金額').fill('1234');
  await form.getByRole('combobox', { name: /科目/ }).click();
  await page.keyboard.insertText('しょうもう');
  await page.keyboard.press('Enter');
  await expect(form.getByRole('combobox', { name: /科目/ })).toHaveValue('消耗品費');
  const wallet = form.getByRole('combobox', { name: /口座/ }).last();
  if (!(await wallet.inputValue()).includes('現金')) {
    await wallet.click();
    await page.keyboard.insertText('現金');
    await page.keyboard.press('Enter');
  }
  await expect(wallet).toHaveValue(/現金/);
  await form.getByLabel('摘要').fill('お試し追記');
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();

  await go(page, '#/journal?q=' + encodeURIComponent('お試し追記'));
  await expect(page.getByRole('row', { name: /お試し追記/ })).toBeVisible();
  await go(page, '#/reports/income-statement?month=2026-09');
  await expect(page.locator('.report-title')).toContainText('2026年9月');
  await expect(supplies.first()).toBeVisible();
  await expect.poll(() => supplies.first().innerText()).not.toBe(before);

  await page.reload();
  await expect(page.locator('main h1')).toBeVisible();
  await go(page, '#/journal?q=' + encodeURIComponent('お試し追記'));
  await expect(page.getByRole('row', { name: /お試し追記/ })).toHaveCount(0);
});

test('AC-14.3 開いてから操作するまで、配信元以外と /api/ はネットワークに出ない', async ({ page }) => {
  const leaked = [];
  const wasm = [];
  page.on('request', (request) => {
    const url = new URL(request.url());
    if (url.origin !== 'http://127.0.0.1:8770') leaked.push(request.url());
    if (url.pathname.includes('/api/')) leaked.push(request.url());
  });
  page.on('response', (response) => {
    if (response.url().includes('.wasm')) wasm.push(response.headers()['content-type'] || '');
  });
  await openDemo(page);
  await page.waitForLoadState('networkidle');
  expect(leaked).toEqual([]);
  expect(wasm.length).toBeGreaterThan(0);
  for (const type of wasm) expect(type).toMatch(/^application\/wasm\b/);
});

test('AC-14.4 仕訳帳のCSV、帳票のCSV、全件のJSONをダウンロードできる', async ({ page }) => {
  await openDemo(page);
  await go(page, '#/settings?section=data');
  const journal = await readDownload(page, () => page.getByRole('link', { name: '仕訳帳のCSV' }).click());
  expect(journal.name).toMatch(/\.csv$/);
  expect(textOf(journal.body)).toContain('日付');

  await go(page, '#/reports/income-statement?month=2026-09');
  const exportLink = page.getByRole('link', { name: 'CSVで書き出す' });
  await expect(exportLink).not.toHaveAttribute('aria-disabled', 'true');
  const report = await readDownload(page, () => exportLink.click());
  expect(report.name).toMatch(/\.csv$/);
  expect(textOf(report.body)).toContain('科目名');

  await go(page, '#/settings?section=data');
  const json = await readDownload(page, () => page.getByRole('link', { name: '全件のデータ（JSON）' }).click());
  expect(json.name).toMatch(/\.json$/);
  expect(textOf(json.body)).toContain('サンプル事務所（デモ）');
});

test('AC-14.5 帳簿の控えは作れない理由と、ソースから動かす代わりを示す', async ({ page }) => {
  await openDemo(page);
  await go(page, '#/settings?section=data');
  const note = page.locator('.backup-unavailable');
  await expect(note).toBeVisible();
  await expect(note).toContainText('作れません');
  await expect(note).toContainText('ソースから');
  await expect(page.getByRole('link', { name: '帳簿の控え（バックアップ）' })).toHaveCount(0);
});

const shots = process.env.KAMOKU_WEB_DEMO_SHOTS || path.join('test-results', 'web-demo-shots');

test('AC-14.6 お試し版の帯は本文が見え、幅を変えても本体を隠さない', async ({ page }) => {
  await openDemo(page);
  const note = page.locator('.banner--web-demo .banner-body > p');
  await expect(note).toContainText('架空の帳簿');
  await expect(note).toContainText('このブラウザの中だけで動き、閉じるか再読み込みで元に戻ります');
  await expect(note).toContainText('税の計算は参考値');
  await expect(note).toContainText('「今日」は帳簿に合わせた2026年9月30日');
  await expect(page.getByRole('link', { name: 'ソース（GitHub）' })).toHaveAttribute(
    'href',
    'https://github.com/haranishi/kamoku',
  );
  for (const width of [390, 768, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    await expect(note).toBeVisible();
    const style = await note.evaluate((el) => {
      const computed = getComputedStyle(el);
      return { position: computed.position, width: computed.width };
    });
    expect(style.position).not.toBe('absolute');
    expect(style.width).not.toBe('1px');
    const overlap = await page.evaluate(() => {
      const banner = document.querySelector('.banner--web-demo').getBoundingClientRect();
      const heading = document.querySelector('main h1').getBoundingClientRect();
      return banner.bottom - heading.top;
    });
    expect(overlap).toBeLessThanOrEqual(1);
    const scroll = await page.evaluate(() => ({
      sw: document.documentElement.scrollWidth,
      cw: document.documentElement.clientWidth,
    }));
    expect(scroll.sw).toBeLessThanOrEqual(scroll.cw + 1);
    // 押せる高さは40px以上。狭い幅では帯を低くする（2つのボタンを1行に並べる）。
    for (const box of await page.locator('.banner--web-demo .banner-actions .btn').evaluateAll((els) =>
      els.map((el) => el.getBoundingClientRect()),
    )) {
      expect(box.height).toBeGreaterThanOrEqual(40);
    }
    if (width <= 480) {
      const tops = await page
        .locator('.banner--web-demo .banner-actions .btn')
        .evaluateAll((els) => els.map((el) => Math.round(el.getBoundingClientRect().top)));
      expect(new Set(tops).size).toBe(1);
    }
    const bannerHeight = await page.locator('.banner--web-demo').evaluate((el) => el.getBoundingClientRect().height);
    console.log(`幅${width}: 案内の帯の高さ ${Math.round(bannerHeight)}px`);
    await page.screenshot({ path: path.join(shots, `web-demo-home-${width}.png`), fullPage: false });
  }
  const reloaded = page.waitForEvent('load');
  await page.getByRole('button', { name: '最初の帳簿に戻す' }).click();
  await reloaded;
  await expect(page.locator('.sidebar-business')).toHaveText('サンプル事務所（デモ）');
});

test('AC-14.8 WASMを読めないときは、日本語の案内が出る', async ({ page }) => {
  await page.route('**/*.wasm', (route) => route.abort());
  await page.goto('./');
  await expect(page.getByRole('alert')).toHaveText(
    '画面を読み込めませんでした。WebAssemblyに対応したブラウザで、ページを再読み込みしてください。',
  );
});

test('AC-14.3 配信先の見出し（CSP など）が付いた応答で動く', async ({ page }) => {
  const response = await page.goto('./');
  const headers = response.headers();
  expect(headers['content-security-policy']).toContain("script-src 'self' 'wasm-unsafe-eval'");
  expect(headers['x-frame-options']).toBe('DENY');
  expect(headers['cross-origin-opener-policy']).toBe('same-origin');
  await expect(page.locator('main h1')).toHaveText('ホーム');
});

test('AC-14.1 配信先が </head> と </body> の前に行を足しても起動する', async ({ page }) => {
  await page.route('**/kamoku-demo/', async (route) => {
    const response = await route.fetch();
    const html = (await response.text())
      .replace('</head>', '  <link rel="stylesheet" href="added-by-host.css">\n</head>')
      .replace('</body>', '  <script type="module" src="added-by-host.js"></script>\n</body>');
    await route.fulfill({ response, body: html });
  });
  await openDemo(page);
  await expect(page.locator('.banner-title')).toHaveText('お試し版です');
});

test('AC-14.3 配る出力に手元の絶対パス・秘密に見える行・25MiB超えのファイルが無い', () => {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const root = process.env.KAMOKU_WEB_DEMO_ROOT || path.resolve(here, '../target/web-demo');
  // 失敗すると終了コードが0でなくなり、execFileSync が例外を投げる。
  const listing = execFileSync('python3', [path.resolve(here, '../tools/check-web-demo-output.py'), root], {
    encoding: 'utf8',
  });
  expect(listing).toContain('web/pkg/kamoku_web_bg.wasm');
});

test('AC-14.4 修飾キーつきのクリックや中ボタンでも、/api/ への通信や新しいタブは出ず、ダウンロードになる', async ({ page, context }) => {
  const opened = [];
  context.on('page', (other) => opened.push(other.url()));
  await openDemo(page);
  await go(page, '#/settings?section=data');
  const link = page.getByRole('link', { name: '全件のデータ（JSON）' });
  for (const options of [
    { modifiers: ['ControlOrMeta'] },
    { modifiers: ['Shift'] },
    { modifiers: ['Alt'] },
    { button: 'middle' },
  ]) {
    const file = await readDownload(page, () => link.click(options));
    expect(file.name, JSON.stringify(options)).toMatch(/\.json$/);
    await expect(link).not.toHaveAttribute('aria-busy', 'true');
  }
  expect(opened).toEqual([]);
  // afterEach が、/api/ への要求が0件であることも確かめる。
});

test('AC-14.8 帳簿の WASM が途中で止まったら、以後の操作に日本語の案内を返す', async ({ page }) => {
  const errors = [];
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  await page.route('**/web/pkg/kamoku_web.js', async (route) => {
    const response = await route.fetch();
    const code = `${await response.text()}
const kamokuOriginalHandle = DemoBook.prototype.handle;
DemoBook.prototype.handle = function (...args) {
  if (globalThis.kamokuBreak) throw new Error('試験で止めた');
  return kamokuOriginalHandle.apply(this, args);
};
`;
    await route.fulfill({ response, body: code });
  });
  await openDemo(page);
  await page.evaluate(() => {
    globalThis.kamokuBreak = true;
  });
  await go(page, '#/journal');
  const message = 'お試し版の帳簿が止まりました。ページを再読み込みしてください。';
  await expect(page.getByText(message).first()).toBeVisible();
  // 止まったあとは、帳簿を呼ばずに同じ案内を返し続ける。
  await page.evaluate(() => {
    globalThis.kamokuBreak = false;
  });
  const status = await page.evaluate(async () => {
    const response = await fetch('/api/v1/status');
    return { code: response.status, body: await response.json() };
  });
  expect(status).toEqual({ code: 500, body: { error: { code: 'internal', message } } });
  expect(errors.some((text) => text.includes('お試し版の帳簿が止まりました'))).toBe(true);
});
