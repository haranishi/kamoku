// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 全画面の、3つの幅 × ライト/ダーク のスクリーンショットを、リポジトリ直下の screens-out/ に保存する（採点に使う）。
// 名前は <画面>-<幅>-<テーマ>.png。今日を 2026-10-14 に固定し、模擬サーバーも同じ日で立てる（毎回同じ絵にするため）。
// 下に固定したバーが途中に写らないよう、表示の高さをページの高さに合わせてから撮る（引き出しとダイアログは画面の高さ）。
// 8767 の帳簿には書き込まない（未確認の途中の様子と無視した行は、一覧の応えを画面の中で差し替えて作る）。初期設定は 8769（--fresh）で撮る。
// 本物のサーバーで撮るときは KAMOKU_URL と KAMOKU_TOKEN、初期設定は空の帳簿のサーバーを KAMOKU_SETUP_SHOTS_URL と
// KAMOKU_SETUP_SHOTS_TOKEN で渡す。本物の合成データは1〜9月なので、月で見る画面は 2026-09 を選ぶ（どちらのサーバーでも同じ月）。

import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { open, entryFields, apiCall } from './helpers.mjs';

const OUT = process.env.KAMOKU_SCREENS_OUT || path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'screens-out');
const FIXTURES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'crates', 'kamoku-import', 'tests', 'fixtures', 'synthetic');
const SETUP_SHOTS = process.env.KAMOKU_SETUP_SHOTS_URL || 'http://127.0.0.1:8769';
const SETUP_SHOTS_TOKEN = process.env.KAMOKU_SETUP_SHOTS_TOKEN || '';
const WIDTHS = [390, 768, 1440];
const THEMES = ['light', 'dark'];

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
  const table = f.preview.locator('table');
  if (page.viewportSize().width >= 640) {
    await expect(table).toBeVisible();
  } else {
    await expect(f.preview.locator('.pv-summary')).toBeVisible();
    const toggle = f.preview.getByRole('button', { name: 'くわしく' });
    await toggle.click();
    await expect(table).toBeVisible();
    await toggle.click();
    await expect(f.preview.locator('.pv-summary')).toBeVisible();
  }
}

// 未確認の途中の様子: 3行に科目が入っていて（1行は按分）、2行が確定候補。書き込まずに見せるため、一覧の応えを差し替える。
async function inboxInProgress(page, { confirm = false } = {}) {
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
  const rows = page.locator('tr.inbox-row');
  await expect(rows.first()).toBeVisible();
  await rows.nth(0).locator('.cand-check').check();
  await rows.nth(1).locator('.cand-check').check();
  if (confirm) {
    await page.keyboard.press('ControlOrMeta+Enter');
    await expect(page.getByRole('dialog', { name: /件を確定しますか/ })).toBeVisible();
  }
}

// 無視した行の一覧: 未確認の行のうち3行を「無視した行」として見せる（帳簿には書き込まない）。
async function inboxIgnored(page) {
  await page.route('**/api/v1/drafts?*', async (route) => {
    const url = route.request().url();
    if (!url.includes('status=ignored')) return route.continue();
    const res = await route.fetch({ url: url.replace('status=ignored', 'status=draft') });
    const body = await res.json();
    body.items = body.items.slice(0, 3).map((d) => ({ ...d, status: 'ignored' }));
    body.total = body.items.length;
    body.counts = { ...body.counts, ignored: body.items.length };
    await route.fulfill({ response: res, json: body });
  });
  await page.reload();
  await expect(page.locator('.ignored-table tbody tr').first()).toBeVisible();
}

const importTo = (step, file) => async (page) => {
  await page.getByLabel('取り込む口座').selectOption({ index: 2 });
  if (step === 1) return;
  await page.locator('input[type="file"]').setInputFiles(path.join(FIXTURES, file));
  await expect(page.locator('.import-map')).toBeVisible();
  if (step === 3) {
    await page.getByRole('button', { name: '確かめる' }).click();
    await expect(page.locator('.check-counts')).toBeVisible();
  }
};

// 初期設定の撮影用の帳簿を、途中まで決めた状態にする（同じ内容なら何もしない）。
async function setupState(page) {
  const s = (await apiCall(page, 'GET', '/settings')).json;
  await apiCall(page, 'PUT', '/settings', { ...s, business_name: 'デモ商店', tax_mode: 'exempt', invoice_number: null, opened_on: '2026-01-01', setup_done: false });
  const have = (await apiCall(page, 'GET', '/accounts')).json.items.filter((a) => a.wallet).map((a) => a.name);
  const add = (name, type, parent, kind) => apiCall(page, 'POST', '/accounts', { name, reading: '', kind, parent, wallet: { type, institution: '' }, default_tax: null, default_pct: 100 });
  if (!have.includes('現金')) await add('現金', 'cash', 'cash', 'asset');
  if (!have.includes('サンプル銀行 普通預金')) await add('サンプル銀行 普通預金', 'bank', 'dep_other', 'asset');
  if (!have.includes('サンプルカード')) await add('サンプルカード', 'card', 'accrued', 'liability');
  const codes = Object.fromEntries((await apiCall(page, 'GET', '/accounts')).json.items.filter((a) => a.wallet).map((a) => [a.name, a.code]));
  await apiCall(page, 'PUT', '/opening', { date: '2026-01-01', balances: [{ account: codes['現金'], amount: 30000 }, { account: codes['サンプル銀行 普通預金'], amount: 820000 }, { account: codes['サンプルカード'], amount: 18400 }] });
}

const SCREENS = [
  { name: 'home', hash: '#/?month=2026-09', ready: '.home-grid' },
  { name: 'entry', hash: '#/entry', ready: '.entry-main', prepare: fillEntry },
  { name: 'journal', hash: '#/journal', ready: '.journal-table tbody tr' },
  {
    name: 'journal-detail',
    hash: '#/journal',
    ready: '.journal-table tbody tr',
    prepare: async (page) => {
      // 訂正のある仕訳（第2版）の詳細を開き、履歴まで見える状態にする。
      const list = (await apiCall(page, 'GET', '/entries?state=all&limit=500')).json.items;
      const revised = list.find((e) => e.revision > 1 && e.state === 'active');
      await page.goto(`/#/journal/${revised.id}`);
      await expect(page.locator('.timeline-item').nth(1)).toBeVisible();
    },
  },
  { name: 'inbox', hash: '#/inbox', ready: '.inbox-bar', prepare: (page) => inboxInProgress(page) },
  { name: 'inbox-confirm', hash: '#/inbox', ready: '.inbox-bar', prepare: (page) => inboxInProgress(page, { confirm: true }) },
  { name: 'inbox-ignored', hash: '#/inbox?status=ignored', ready: 'main h1', prepare: inboxIgnored },
  { name: 'import-1', hash: '#/import', ready: '.drop-zone', prepare: importTo(1) },
  { name: 'import-2', hash: '#/import', ready: '.drop-zone', prepare: importTo(2, 'sjis.csv') },
  { name: 'import-3', hash: '#/import', ready: '.drop-zone', prepare: importTo(3, 'utf8.csv') },
  { name: 'reports-monthly', hash: '#/reports/monthly', ready: '.chart-svg' },
  { name: 'reports-income', hash: '#/reports/income-statement', ready: '.report-table' },
  { name: 'reports-balance', hash: '#/reports/balance-sheet', ready: '.bs-grid' },
  { name: 'reports-trial', hash: '#/reports/trial-balance', ready: '.report-table' },
  { name: 'reports-ledger', hash: '#/reports/ledger?account=w03&month=2026-09', ready: '.ledger-table' },
  { name: 'reports-tax', hash: '#/reports/tax-summary', ready: '.tax-sums' },
  { name: 'wallets', hash: '#/wallets', ready: '.wallet-list' },
  {
    name: 'wallets-add',
    hash: '#/wallets',
    ready: '.wallet-list',
    prepare: async (page) => {
      await expect(page.locator('.wallet-list').first()).toBeVisible();
      await page.getByRole('button', { name: '口座を追加' }).click();
      await expect(page.getByRole('dialog', { name: '口座を追加' })).toBeVisible();
    },
  },
  { name: 'accounts', hash: '#/accounts', ready: '.accounts-table' },
  { name: 'partners', hash: '#/partners', ready: '.partners-table' },
  { name: 'settings', hash: '#/settings', ready: '.settings-section' },
  ...[1, 2, 3, 4].map((n) => ({ name: `setup-${n}`, base: SETUP_SHOTS, hash: `#/setup?step=${n}`, ready: '.setup-card', before: setupState })),
];

for (const width of WIDTHS) {
  for (const theme of THEMES) {
    for (const s of SCREENS) {
      test(`${s.name}-${width}-${theme}`, async ({ page }) => {
        await page.clock.setFixedTime(new Date('2026-10-14T10:00:00+09:00'));
        await page.emulateMedia({ colorScheme: theme, reducedMotion: 'reduce' });
        await page.setViewportSize({ width, height: 900 });
        if (s.base) {
          if (SETUP_SHOTS_TOKEN) await page.goto(`${s.base}/?t=${SETUP_SHOTS_TOKEN}`);
          await page.goto(`${s.base}/#/`);
          await s.before(page);
          await page.goto(`${s.base}/${s.hash}`);
          await page.reload();
        } else {
          await open(page, s.hash);
        }
        if (s.prepare) await s.prepare(page);
        else await expect(page.locator(s.ready).first()).toBeVisible();
        // 読み込みが済み、ページの高さが2回続けて同じになってから測る（途中の高さで撮ると、下が切れたり、下に余白が出たりした）。
        await page.waitForLoadState('networkidle');
        const pageHeight = () => page.evaluate(() => Math.ceil(document.documentElement.scrollHeight));
        let height = await pageHeight();
        for (let i = 0; i < 5; i += 1) {
          await page.waitForTimeout(150);
          const now = await pageHeight();
          if (now === height) break;
          height = now;
        }
        const dialog = page.locator('dialog[open]');
        if (await dialog.count()) {
          // 引き出しとダイアログは画面の高さで撮る（中身は、その中でスクロールする）。
          await page.setViewportSize({ width, height: 900 });
        } else {
          await page.setViewportSize({ width, height: Math.max(900, height) });
          // 広げたあとに伸びたら（画面の高さで決まる部分があるとき）、もう一度合わせる。
          const grown = await pageHeight();
          if (grown > Math.max(900, height)) await page.setViewportSize({ width, height: grown });
        }
        await page.waitForTimeout(200);
        await page.screenshot({ path: path.join(OUT, `${s.name}-${width}-${theme}.png`) });
      });
    }
  }
}
