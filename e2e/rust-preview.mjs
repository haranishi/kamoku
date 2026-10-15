// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test, expect } from '@playwright/test';
import { mkdir, writeFile, stat } from 'node:fs/promises';
import { apiCall } from './helpers.mjs';
import { readinessSamples } from './rust-ui-metrics.mjs';
import { expectJournalTextFits } from './rust-ui-layout.mjs';

const PREVIEW = '/rust/index.html';
const OUT = 'rust-ui-results';
const links = (page) => page.locator('tbody a[data-entry-id]');
const search = (page) => page.getByRole('searchbox', { name: '文字で探す' });
const ready = (page) => expect(page.locator('.table-card')).toHaveAttribute('aria-busy', 'false');

async function authenticate(page) {
  // Authenticate outside Playwright navigation so a failing goto/trace cannot
  // print or record the token-bearing URL. Only synthetic UI/metrics are saved.
  let cookie;
  try {
    const response = await fetch(`${process.env.KAMOKU_URL}/?t=${process.env.KAMOKU_TOKEN}`, { redirect: 'manual' });
    if (response.status !== 303) throw new Error('auth');
    cookie = response.headers.get('set-cookie')?.match(/^kamoku_session=([^;]+);/)?.[1];
  } catch {
    throw new Error('Cannot authenticate the synthetic Rust UI test server.');
  }
  if (!cookie) throw new Error('Synthetic test server did not issue a session.');
  await page.context().addCookies([{ name: 'kamoku_session', value: cookie, url: process.env.KAMOKU_URL, httpOnly: true, sameSite: 'Strict' }]);
  page.kamokuAuthed = true;
}

async function openPreview(page, query = '') {
  await page.goto(`${PREVIEW}${query}`);
  await expect(page.getByRole('heading', { name: '仕訳帳', exact: true })).toBeVisible();
  await ready(page);
}

async function createEntry(page, description, amount = 500) {
  const result = await apiCall(page, 'POST', '/entries', {
    kind: 'expense', date: '2026-10-02', amount, account: 'exp_supplies',
    wallet: 'w01', tax_code: 'P10', partner: '合成データ商店', description,
    business_pct: 100, memo: '',
  });
  expect(result.status).toBe(201);
  return result.json;
}

test.beforeEach(async ({ page }) => {
  await authenticate(page);
  await openPreview(page);
});

test('actual WASM starts under CSP and uses the streaming MIME type without JS eval', async ({ page }) => {
  const wasm = await page.request.get('/rust/pkg/kamoku_ui_bg.wasm');
  expect(wasm.status()).toBe(200);
  expect(wasm.headers()['content-type']).toBe('application/wasm');
  expect((await wasm.body()).subarray(0, 4)).toEqual(Buffer.from([0, 97, 115, 109]));
  const response = await page.request.get(PREVIEW);
  const policy = response.headers()['content-security-policy'];
  expect(policy).toContain("'wasm-unsafe-eval'");
  expect(policy.split(/\s+/)).not.toContain("'unsafe-eval'");
  // Automation evaluation can bypass CSP. Run the probe as an actual same-origin
  // external script, then use evaluation only to read its result.
  await page.route('**/rust/csp-probe.js', (route) => route.fulfill({
    contentType: 'text/javascript',
    body: `globalThis.kamokuCspProbe = { ran: true, blocked: false };
      try { globalThis.eval('1 + 1'); }
      catch (error) { globalThis.kamokuCspProbe.blocked = error.name === 'EvalError'; }`,
  }));
  await page.addScriptTag({ url: '/rust/csp-probe.js' });
  const probe = await page.evaluate(() => globalThis.kamokuCspProbe);
  expect(probe).toEqual({ ran: true, blocked: true });
  await page.unroute('**/rust/csp-probe.js');
  await expect(page.getByRole('table')).toBeVisible();
  await expect(page.getByRole('link', { name: '通常の仕訳帳へ' })).toHaveAttribute('href', '/#/journal');
});

test('search, exact sums, bounded pages and date/state filters agree with the real API', async ({ page }) => {
  const prefix = 'RustPaging合成';
  const entries = [];
  for (let i = 0; i < 52; i++) entries.push(await createEntry(page, `${prefix} ${i}`, 100 + i));
  const canceled = await apiCall(page, 'POST', `/entries/${entries[0].id}/void`, { reason: '合成試験の取消', expected_revision: 1 });
  expect(canceled.status).toBe(200);
  await search(page).fill(prefix);
  await search(page).press('Enter');
  await ready(page);
  await expect(links(page)).toHaveCount(50);
  await expect(page.getByRole('status')).toContainText('52件のうち 1〜50件目');
  await expect(page.getByRole('status')).toContainText('6,526円');
  await page.getByRole('button', { name: '次の50件' }).click();
  await ready(page);
  await expect(page).toHaveURL(/page=2/);
  await expect(links(page)).toHaveCount(2);
  await expect(links(page).first()).toBeFocused();
  await page.getByLabel('状態', { exact: true }).selectOption('active');
  await ready(page);
  await expect(page).not.toHaveURL(/page=2/);
  await expect(page.getByRole('status')).toContainText('51件');
  await expect(page.getByRole('status')).toContainText('6,426円');
  await page.getByLabel('状態', { exact: true }).selectOption('voided');
  await ready(page);
  await expect(links(page)).toHaveCount(1);
  await expect(page.getByRole('status')).toContainText('100円');
  await page.getByLabel('期間の始め', { exact: true }).fill('2026-10-03');
  await page.getByLabel('期間の始め', { exact: true }).press('Tab');
  await ready(page);
  await expect(links(page)).toHaveCount(0);
  await expect(page.getByText('条件に合う仕訳はありません。', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: '条件を消す' }).click();
  await ready(page);
  await expect(page).toHaveURL(new RegExp(`${PREVIEW}$`));
});

test('query history and the focused detail link survive detail navigation and reload', async ({ page }) => {
  const entry = await createEntry(page, 'Rust履歴 A&B + 電車');
  await search(page).fill('A&B + 電車');
  await search(page).press('Enter');
  await ready(page);
  const queryUrl = page.url();
  await search(page).press('ArrowDown');
  await expect(links(page).first()).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(new RegExp(`/#/journal/${entry.id}`));
  await expect(page.getByRole('dialog')).toContainText('Rust履歴 A&B + 電車');
  await page.goBack();
  await ready(page);
  await expect(page).toHaveURL(queryUrl);
  await expect(search(page)).toHaveValue('A&B + 電車');
  await expect(links(page).first()).toBeFocused();
  await page.reload();
  await ready(page);
  await expect(links(page).first()).toBeFocused();
  await search(page).fill('一致しない合成検索');
  await search(page).press('Enter');
  await ready(page);
  await expect(links(page)).toHaveCount(0);
  await page.goBack();
  await ready(page);
  await expect(search(page)).toHaveValue('A&B + 電車');
  await expect(links(page).first()).toBeFocused();
});

test('composition Enter and held Enter never search, and character shortcuts can be disabled', async ({ page }) => {
  let requests = 0;
  page.on('request', (request) => { if (request.url().includes('/api/v1/entries?')) requests++; });
  await search(page).fill('変換中の合成データ');
  await search(page).evaluate((el) => {
    el.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true }));
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', keyCode: 13, isComposing: true, bubbles: true, cancelable: true }));
    el.dispatchEvent(new CompositionEvent('compositionend', { bubbles: true }));
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', keyCode: 13, bubbles: true, cancelable: true }));
    el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', keyCode: 229, bubbles: true, cancelable: true }));
  });
  await expect(page).toHaveURL(new RegExp(`${PREVIEW}$`));
  expect(requests).toBe(0);
  await page.waitForTimeout(60);
  await search(page).evaluate((el) => el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', repeat: true, bubbles: true, cancelable: true })));
  expect(requests).toBe(0);
  await search(page).press('Enter');
  await ready(page);
  await expect(page).toHaveURL(/q=/);
  await page.getByRole('checkbox', { name: '1文字のショートカットを使う' }).uncheck();
  await page.locator('main').focus();
  await page.keyboard.press('/');
  await expect(search(page)).not.toBeFocused();
  await page.getByRole('checkbox', { name: '1文字のショートカットを使う' }).check();
  await page.locator('main').focus();
  await page.keyboard.press('/');
  await expect(search(page)).toBeFocused();
});

test('late responses cannot replace a newer search and a failed request can be retried', async ({ page }) => {
  await createEntry(page, 'Rust新しい応答');
  let release;
  const held = new Promise((resolve) => { release = resolve; });
  let seen;
  const started = new Promise((resolve) => { seen = resolve; });
  await page.route('**/api/v1/entries?**', async (route) => {
    if (new URL(route.request().url()).searchParams.get('q') === '遅い検索') {
      seen();
      await held;
    }
    await route.continue();
  });
  await search(page).fill('遅い検索');
  await search(page).press('Enter');
  await started;
  await search(page).fill('Rust新しい応答');
  await search(page).press('Enter');
  await ready(page);
  await expect(links(page)).toHaveCount(1);
  const late = page.waitForResponse((response) => new URL(response.url()).searchParams.get('q') === '遅い検索');
  release();
  await late;
  await expect(page.locator('tbody')).toContainText('Rust新しい応答');
  await page.unrouteAll({ behavior: 'wait' });
  await page.route('**/api/v1/entries?**', (route) => route.abort());
  await search(page).fill('通信の失敗');
  await search(page).press('Enter');
  await expect(page.getByRole('alert')).toContainText('サーバーにつながりません');
  await expect(links(page)).toHaveCount(0);
  await page.unrouteAll({ behavior: 'wait' });
  await page.getByRole('button', { name: 'もう一度読み込む' }).click();
  await ready(page);
  await expect(page.getByRole('alert')).toBeHidden();
});

test('untrusted descriptions remain text and large yen totals retain integer precision', async ({ page }) => {
  const description = '<img src=x onerror="window.rustInjection=true"> Rust安全文字';
  await createEntry(page, description);
  await search(page).fill('Rust安全文字');
  await search(page).press('Enter');
  await ready(page);
  await expect(page.locator('tbody')).toContainText(description);
  await expect(page.locator('tbody img')).toHaveCount(0);
  expect(await page.evaluate(() => window.rustInjection)).toBeUndefined();
  await page.route('**/api/v1/entries?**', async (route) => {
    const response = await route.fetch();
    const body = await response.text();
    await route.fulfill({ response, body: body.replace(/"sum_amount":\s*\d+/, '"sum_amount":9007199254740993') });
  });
  await page.reload();
  await ready(page);
  await expect(page.getByRole('status')).toContainText('9,007,199,254,740,993円');
});

test('synthetic screenshots and fresh-context/search/asset evidence at three widths', async ({ page, browser }) => {
  await mkdir(OUT, { recursive: true });
  const readiness = await readinessSamples(browser, authenticate);
  await openPreview(page);
  const samples = [];
  for (let i = 0; i < 20; i++) {
    await search(page).fill(i % 2 ? '' : '合成');
    const elapsed = await search(page).evaluate(async (el) => {
      const start = performance.now();
      const card = document.querySelector('.table-card');
      const finished = new Promise((resolve) => {
        const observer = new MutationObserver(() => {
          if (card.getAttribute('aria-busy') === 'false') {
            observer.disconnect();
            requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now() - start)));
          }
        });
        observer.observe(card, { attributes: true, attributeFilter: ['aria-busy'] });
      });
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
      return finished;
    });
    samples.push(elapsed);
  }
  await createEntry(page, 'Rust金額上限 合成データの仕訳 A', 9_999_999_999_999);
  await createEntry(page, 'Rust金額上限 合成データの仕訳 B', 9_999_999_999_999);
  await search(page).fill('Rust金額上限');
  await search(page).press('Enter');
  await ready(page);
  await expect(page.getByRole('status')).toContainText('19,999,999,999,998円');
  await expect(page.locator('tbody .amount')).toHaveText(['9,999,999,999,999円', '9,999,999,999,999円']);
  for (const width of [390, 768, 1440]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect(page.getByRole('table')).toBeVisible();
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth > innerWidth);
    expect(overflow).toBe(false);
    await expectJournalTextFits(page, width);
    await expect(page.getByRole('columnheader', { name: '日付' })).toHaveCount(1);
    await page.screenshot({ path: `${OUT}/journal-${width}.png`, fullPage: true });
  }
  const sorted = [...samples].sort((a, b) => a - b);
  const assets = {};
  for (const file of ['kamoku_ui.js', 'kamoku_ui_bg.wasm']) assets[file] = (await stat(`../ui/rust/pkg/${file}`)).size;
  await writeFile(`${OUT}/metrics.json`, JSON.stringify({
    dataset: 'synthetic demo plus browser-test records; CI runner',
    builds: 'debug Rust server; release WASM UI',
    baselineJournalReadyMs: readiness.baseline,
    rustJournalReadyMs: readiness.rust,
    rustSearchToPaintMs: { samples, median: (sorted[9] + sorted[10]) / 2, p95: sorted[18] },
    assetsBytes: assets,
    limitations: ['Five alternating pairs of fresh HTTP-cache contexts; browser process, OS and server stay warm.', 'One CI machine/browser; not a benchmark of typical customer hardware.', 'Readiness includes Playwright polling; do not treat it as precise user latency.'],
  }, null, 2));
});
