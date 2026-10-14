// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// READMEと「画面の紹介」に載せる画像を作る（本物のデモのサーバーを、合成データのまま撮る）。
//   e2e/run-real.sh docs-images      → docs/images/ に出る（KAMOKU_DOC_IMAGES_OUT で変えられる）
// 画面の高さで切り取る（縦に長い画面の全体ではなく、最初の1画面）。今日は 2026-10-14 に固定する。

import { chromium, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { open, entryFields, apiCall, walletOf } from './helpers.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const OUT = process.env.KAMOKU_DOC_IMAGES_OUT || path.resolve(HERE, '..', 'docs', 'images');
const FIXTURE = path.resolve(HERE, '..', 'crates', 'kamoku-import', 'tests', 'fixtures', 'synthetic', 'sjis.csv');

async function fillEntry(page) {
  const f = entryFields(page);
  await f.amount.fill('11000');
  await f.account.click();
  await page.keyboard.insertText('通信費');
  await page.keyboard.press('Enter');
  await f.wallet.click();
  await page.keyboard.insertText('サンプルカード');
  await page.keyboard.press('Enter');
  await f.partner.click();
  await page.keyboard.insertText('サンプル通信');
  await f.description.fill('10月分 携帯電話');
  await f.description.blur();
  await expect(f.preview.locator('table')).toBeVisible();
}

// 未確認の途中の様子: 3行に科目が入り、2行が確定候補。書き込まずに見せるため、一覧の応えを差し替える。
async function inboxInProgress(page) {
  const preset = [
    { account: 'exp_supplies', tax_code: 'P10', business_pct: 100, partner: 'サンプル文具' },
    { account: 'exp_travel', tax_code: null, business_pct: 100, partner: '' },
    { account: 'exp_comm', tax_code: 'P10X', business_pct: 70, partner: 'サンプル通信', memo: '10月分' },
  ];
  await page.route('**/api/v1/drafts?*', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.items = body.items.map((d, i) => (preset[i] ? { ...d, ...preset[i] } : d));
    await route.fulfill({ response: res, json: body });
  });
  await page.reload();
  const rows = page.locator('tr.inbox-row, .inbox-card');
  await expect(rows.first()).toBeVisible();
  const checks = page.locator('.cand-check');
  await checks.nth(0).check();
  await checks.nth(1).check();
}

async function revisedEntry(page) {
  const list = (await apiCall(page, 'GET', '/entries?state=all&limit=500')).json.items;
  const revised = list.find((e) => e.revision > 1 && e.state === 'active');
  await page.goto(`/#/journal/${revised.id}`);
  await expect(page.locator('.timeline-item').nth(1)).toBeVisible();
}

async function importStep2(page) {
  const w = await walletOf(page, 'bank');
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await page.locator('input[type="file"]').setInputFiles(FIXTURE);
  await expect(page.locator('.import-map')).toBeVisible();
}

// 名前、幅、高さ、テーマ、開く場所、準備。
const SHOTS = [
  { name: 'home', w: 1440, h: 900, hash: '#/?month=2026-09', ready: '.home-grid' },
  { name: 'entry', w: 1440, h: 900, hash: '#/entry', ready: '.entry-main', prepare: fillEntry },
  { name: 'inbox', w: 1440, h: 900, hash: '#/inbox', ready: '.inbox-bar', prepare: inboxInProgress },
  { name: 'journal', w: 1440, h: 900, hash: '#/journal', ready: '.journal-table tbody tr' },
  { name: 'journal-detail', w: 1440, h: 900, hash: '#/journal', ready: '.journal-table tbody tr', prepare: revisedEntry },
  { name: 'import', w: 1440, h: 900, hash: '#/import', ready: '.drop-zone', prepare: importStep2 },
  { name: 'monthly', w: 1440, h: 900, hash: '#/reports/monthly', ready: '.chart-svg' },
  { name: 'balance', w: 1440, h: 900, hash: '#/reports/balance-sheet', ready: '.bs-grid' },
  { name: 'trial', w: 1440, h: 900, hash: '#/reports/trial-balance', ready: '.report-table' },
  { name: 'settings', w: 1440, h: 900, hash: '#/settings', ready: '.settings-section' },
  { name: 'home-dark', w: 1440, h: 900, theme: 'dark', hash: '#/?month=2026-09', ready: '.home-grid' },
  { name: 'home-mobile', w: 390, h: 844, hash: '#/?month=2026-09', ready: '.home-grid' },
  { name: 'entry-mobile', w: 390, h: 844, hash: '#/entry', ready: '.entry-main', prepare: fillEntry },
  { name: 'inbox-mobile', w: 390, h: 844, hash: '#/inbox', ready: '.inbox-bar', prepare: inboxInProgress },
];

const only = process.argv.slice(2);
const browser = await chromium.launch();
try {
  for (const s of SHOTS) {
    if (only.length && !only.includes(s.name)) continue;
    const context = await browser.newContext({
      baseURL: process.env.KAMOKU_URL,
      viewport: { width: s.w, height: s.h },
      locale: 'ja-JP',
      timezoneId: 'Asia/Tokyo',
      colorScheme: s.theme || 'light',
      reducedMotion: 'reduce',
    });
    const page = await context.newPage();
    await page.clock.setFixedTime(new Date('2026-10-14T10:00:00+09:00'));
    await open(page, s.hash);
    if (s.prepare) await s.prepare(page);
    else await expect(page.locator(s.ready).first()).toBeVisible();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(400);
    await page.screenshot({ path: path.join(OUT, `${s.name}.png`) });
    console.log(`撮りました: ${s.name}.png（${s.w}x${s.h}）`);
    await context.close();
  }
} finally {
  await browser.close();
}
