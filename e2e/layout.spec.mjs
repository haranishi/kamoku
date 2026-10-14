// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// AC-11.1 幅390・768・1440で、ページ全体に横スクロールが出ない（表の中は除く）。
// U1 の3画面に、U2 の全画面（未確認・取込の3段階・帳票の6タブ・口座・勘定科目・取引先・設定・初期設定の4段階）を足した。

import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { open, openSetup, apiCall, walletOf, SETUP_URL } from './helpers.mjs';

const FIXTURE = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'crates', 'kamoku-import', 'tests', 'fixtures', 'synthetic', 'utf8.csv');

/** 取込の(2)か(3)まで進める。 */
const importTo = (step) => async (page) => {
  const w = await walletOf(page, 'bank');
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await page.locator('input[type="file"]').setInputFiles(FIXTURE);
  await expect(page.locator('.import-map')).toBeVisible();
  if (step === 3) {
    await page.getByRole('button', { name: '確かめる' }).click();
    await expect(page.locator('.check-counts')).toBeVisible();
  }
};

/** 元帳は、科目（カードの口座）を選んだ状態で見る。 */
async function ledgerWithAccount(page) {
  const w = await walletOf(page, 'card');
  await page.goto(`/#/reports/ledger?account=${w.code}`);
  await expect(page.locator('.ledger-table')).toBeVisible();
}

/** 初期設定の段階は、空の帳簿のサーバーで、済んでいない状態にして見る。 */
async function setupNotDone(page) {
  const s = (await apiCall(page, 'GET', '/settings')).json;
  if (s.setup_done) {
    await apiCall(page, 'PUT', '/settings', { ...s, setup_done: false });
    await page.reload();
  }
}

export const SCREENS = [
  { name: 'home', hash: '#/', ready: '.next-card' },
  { name: 'entry', hash: '#/entry', ready: '.entry-main' },
  { name: 'journal', hash: '#/journal', ready: '.journal-table tbody tr' },
  { name: 'inbox', hash: '#/inbox', ready: '.inbox-bar' },
  { name: 'inbox-ignored', hash: '#/inbox?status=ignored', ready: '.inbox-tools' },
  { name: 'import', hash: '#/import', ready: '.drop-zone' },
  { name: 'import-columns', hash: '#/import', ready: '.import-map', prepare: importTo(2) },
  { name: 'import-check', hash: '#/import', ready: '.check-counts', prepare: importTo(3) },
  { name: 'reports-monthly', hash: '#/reports/monthly', ready: '.chart-svg' },
  { name: 'reports-income', hash: '#/reports/income-statement', ready: '.report-table' },
  { name: 'reports-range', hash: `#/reports/trial-balance?from=${new Date().getFullYear()}-04-01&to=${new Date().getFullYear()}-06-30`, ready: '.period-range' },
  { name: 'reports-balance', hash: '#/reports/balance-sheet', ready: '.bs-grid' },
  { name: 'reports-trial', hash: '#/reports/trial-balance', ready: '.report-table' },
  { name: 'reports-ledger', hash: '#/reports/ledger', ready: '.report-content', prepare: ledgerWithAccount },
  { name: 'reports-tax', hash: '#/reports/tax-summary', ready: '.tax-sums' },
  { name: 'wallets', hash: '#/wallets', ready: '.wallet-list' },
  { name: 'accounts', hash: '#/accounts', ready: '.accounts-table' },
  { name: 'partners', hash: '#/partners', ready: '.partners-table' },
  { name: 'settings', hash: '#/settings', ready: '.settings-section' },
  ...[1, 2, 3, 4].map((n) => ({ name: `setup-${n}`, base: SETUP_URL, hash: `#/setup?step=${n}`, ready: '.setup-card', setup: true, prepare: setupNotDone })),
];
export const WIDTHS = [390, 768, 1440];

for (const width of WIDTHS) {
  for (const s of SCREENS) {
    test(`AC-11.1 ${s.name} は幅${width}pxで横にスクロールしない`, async ({ page }) => {
      await page.setViewportSize({ width, height: 900 });
      if (s.base) await openSetup(page, s.hash);
      else await open(page, s.hash);
      if (s.prepare) await s.prepare(page);
      await expect(page.locator(s.ready).first()).toBeVisible();
      const [scroll, client] = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
      expect(scroll).toBeLessThanOrEqual(client);
      // 共通の枠: 1024px以上はサイドバー、768〜1023pxは閉じられるサイドバー（広げるボタンあり）、768px未満は下のバー。
      // 初期設定の間は、ほかの画面へ行けないので、どの幅でも移動のメニューを隠す。
      const shell = await page.evaluate(() => {
        const shown = (sel) => {
          const el = document.querySelector(sel);
          return Boolean(el) && getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
        };
        return { sidebar: shown('.sidebar'), railToggle: shown('.rail-toggle'), bottomnav: shown('.bottomnav') };
      });
      const none = { sidebar: false, railToggle: false, bottomnav: false };
      expect(shell).toEqual(
        s.setup ? none : width >= 1024 ? { sidebar: true, railToggle: false, bottomnav: false } : width >= 768 ? { sidebar: true, railToggle: true, bottomnav: false } : { sidebar: false, railToggle: false, bottomnav: true },
      );
      // 768px未満で押せる場所は、44×44px以上（nfr.md）。
      if (width < 768) {
        const small = await page.evaluate(() =>
          [...document.querySelectorAll('main button, main a.btn, main .seg-option, .bottomnav-item, .topbar button')]
            .filter((el) => el.offsetParent !== null)
            .map((el) => ({ text: el.textContent.trim() || el.getAttribute('aria-label'), r: el.getBoundingClientRect() }))
            .filter(({ r }) => r.width < 44 || r.height < 44)
            .map(({ text, r }) => `${text} ${Math.round(r.width)}x${Math.round(r.height)}`),
        );
        expect(small).toEqual([]);
      }
    });
  }
}

// 画面を開いて、準備を済ませ、読み込みが終わるまで待つ（AC-11.6 と AC-11.7 の試験で使う）。
async function gotoScreen(page, s, width) {
  await page.setViewportSize({ width, height: 900 });
  if (s.base) await openSetup(page, s.hash);
  else await open(page, s.hash);
  if (s.prepare) await s.prepare(page);
  await expect(page.locator(s.ready).first()).toBeVisible();
  await page.waitForLoadState('networkidle');
}

for (const width of WIDTHS) {
  for (const s of SCREENS) {
    test(`AC-11.6 ${s.name} は幅${width}pxで、横に隠れる列も、「…」で欠ける文字も無い`, async ({ page }) => {
      await gotoScreen(page, s, width);
      const bad = await page.evaluate(() => {
        const visible = (e) => {
          const r = e.getBoundingClientRect();
          return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden';
        };
        const name = (e) => `${e.tagName.toLowerCase()}.${String(e.className).split(' ')[0]}`;
        // 横にスクロールする枠の中に、見えていない部分がある（列が隠れている）。意図して許すものは data-allow-hscroll を付ける。
        const hidden = [...document.querySelectorAll('main *')]
          .filter((e) => /(auto|scroll)/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && visible(e) && !e.closest('[data-allow-hscroll]'))
          .map((e) => `${name(e)} ${e.scrollWidth}>${e.clientWidth}`);
        // 「…」で省略されて、情報が欠けている。
        const cut = [...document.querySelectorAll('main *, .sidebar *, .topbar *, .bottomnav *')]
          .filter((e) => getComputedStyle(e).textOverflow === 'ellipsis' && e.scrollWidth > e.clientWidth + 1 && visible(e))
          .map((e) => `${name(e)}「${e.textContent.trim().slice(0, 12)}」`);
        return { hidden, cut };
      });
      expect(bad.hidden, '横に隠れている列がある').toEqual([]);
      expect(bad.cut, '「…」で欠けた文字がある').toEqual([]);
    });
  }
}

for (const width of WIDTHS.filter((w) => w < 1024)) {
  for (const s of SCREENS) {
    test(`AC-11.7 ${s.name} は幅${width}pxで、押せる場所がどれも44×44px以上ある`, async ({ page }) => {
      await gotoScreen(page, s, width);
      const small = await page.evaluate(() => {
        const visible = (e) => {
          const r = e.getBoundingClientRect();
          return r.width > 0 && r.height > 0 && getComputedStyle(e).visibility !== 'hidden' && !e.closest('[hidden]');
        };
        // 文章の中に埋まったリンク（まわりに地の文字がある、行の中のリンク）は、44pxを求めない。
        const inText = (e) => {
          if (getComputedStyle(e).display !== 'inline') return false;
          return [...e.parentElement.childNodes].some((n) => n !== e && n.nodeType === 3 && n.textContent.trim() !== '');
        };
        return [...document.querySelectorAll('a[href], button, summary, select, [role="button"], [role="tab"], input[type="checkbox"], input[type="radio"]')]
          .filter((e) => visible(e) && !e.classList.contains('skip-link') && !inText(e))
          // チェックとラジオは、包むラベルを含めた大きさで数える。
          .map((e) => ({ e, r: (e.matches('input') ? e.closest('label') || e : e).getBoundingClientRect() }))
          .filter(({ r }) => r.width < 44 || r.height < 44)
          .map(({ e, r }) => `${e.tagName.toLowerCase()}.${String(e.className).split(' ')[0]}「${(e.textContent || e.getAttribute('aria-label') || '').trim().slice(0, 10)}」${Math.round(r.width)}x${Math.round(r.height)}`);
      });
      expect(small).toEqual([]);
    });
  }
}

// SC-03 下の固定バーの金額は、大きな金額でも切れない（390・768・1440で確かめる）。
for (const width of WIDTHS) {
  test(`SC-03 未確認の下の固定バーは、幅${width}pxで大きな金額でも切れず、画面の外に出ない`, async ({ page }) => {
    const big = [9876543210, 1234567890123, 45000000];
    const N = 24;
    // 行は、画面の高さを超える数を作る（バーの下に隠れうる行があるように）。保存は送らない操作だけをする。
    await page.route('**/api/v1/drafts?*', async (route) => {
      const res = await route.fetch();
      const body = await res.json();
      const base = body.items[0];
      body.items = Array.from({ length: N }, (_, i) => ({ ...base, id: 900000 + i, description: `配置の確認 ${i + 1}`, account: 'exp_supplies', amount: big[i] ?? 1000 + i, direction: i === 2 ? 'in' : 'out' }));
      body.total = N;
      await route.fulfill({ response: res, json: body });
    });
    await page.setViewportSize({ width, height: 800 });
    await open(page, '#/inbox');
    const rows = page.locator('tr.inbox-row');
    await expect(rows).toHaveCount(N);
    for (let i = 0; i < 3; i += 1) await rows.nth(i).locator('.cand-check').check();
    const bar = page.locator('.inbox-bar');
    await expect(bar).toContainText('確定候補3件');
    await expect(bar).toContainText('1,244,444,433,333');
    await expect(bar).toContainText('45,000,000');
    const cut = await page.evaluate(() =>
      [...document.querySelectorAll('.inbox-sum dd, .inbox-sum dt, .inbox-confirm')]
        .filter((el) => {
          const r = el.getBoundingClientRect();
          return el.scrollWidth > el.clientWidth + 1 || r.right > window.innerWidth || r.left < 0;
        })
        .map((el) => el.textContent),
    );
    expect(cut).toEqual([]);
    const barBox = await bar.boundingBox();
    expect(barBox.x).toBeGreaterThanOrEqual(0);
    expect(barBox.x + barBox.width).toBeLessThanOrEqual(width);
    const [scroll, client] = await page.evaluate(() => [document.documentElement.scrollWidth, document.documentElement.clientWidth]);
    expect(scroll).toBeLessThanOrEqual(client);
    // AC-11.8 バーの下の端と、その下（下のバーか画面の端）の間から、後ろの本文がのぞかない。
    const peek = await page.evaluate(() => {
      const r = document.querySelector('.inbox-bar').getBoundingClientRect();
      const nav = document.querySelector('.bottomnav');
      const navBox = nav ? nav.getBoundingClientRect() : null;
      const floor = navBox && navBox.width > 0 ? navBox.top : window.innerHeight;
      const gap = Math.round(floor - r.bottom);
      if (gap <= 0) return { gap, hidden: true };
      const el = document.elementFromPoint(r.left + r.width / 2, r.bottom + Math.min(gap, 4) / 2);
      return { gap, hidden: Boolean(el && el.closest('.inbox-bar, .bottomnav, [data-bar-mask]')), seen: el && String(el.className) };
    });
    expect(peek.hidden, `バーの下に ${peek.gap}px の隙間があり、後ろの本文（${peek.seen}）がのぞく`).toBe(true);
    // Enterで移った先の欄は、固定のバーの下に隠れない。次の行の科目の欄が、画面の中でちょうどバーの裏に来る位置まで
    // スクロールしてから、今の行で Enter を押す（画面の外の欄は、フォーカスで真ん中へ動くので確かめにならない）。
    const k = N - 4;
    await page.evaluate((i) => {
      const inputs = document.querySelectorAll('tr.inbox-row td.c-account input');
      const top = document.querySelector('.inbox-bar').getBoundingClientRect().top;
      window.scrollBy(0, inputs[i + 1].getBoundingClientRect().top - (top + 8));
    }, k);
    const behindBar = await page.evaluate((i) => {
      const r = document.querySelectorAll('tr.inbox-row td.c-account input')[i + 1].getBoundingClientRect();
      return r.top > document.querySelector('.inbox-bar').getBoundingClientRect().top && r.bottom <= window.innerHeight;
    }, k);
    expect(behindBar).toBe(true);
    await rows.nth(k).locator('td.c-account input').focus();
    await page.keyboard.press('Enter');
    await expect(rows.nth(k + 1).locator('td.c-account input')).toBeFocused();
    const [focusBottom, barTop] = await page.evaluate(() => [document.activeElement.getBoundingClientRect().bottom, document.querySelector('.inbox-bar').getBoundingClientRect().top]);
    expect(focusBottom).toBeLessThanOrEqual(barTop);
  });
}
