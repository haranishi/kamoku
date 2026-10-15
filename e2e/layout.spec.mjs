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
    await expect(bar).toContainText('確定する行3件');
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

// SC-02 キーボードだけで動くとき、フォーカスのある欄は、上に固定した帯の裏にも、下に固定した帯の裏にも隠れない
// （ページ全体の scroll-padding。layout.css）。Tab で本文の終わりまで進み、Shift+Tab で1つずつ戻る。
// 640×450 は文字を200%に拡大した画面に近い。高さ520px以下では下のバーを隠す（隠した帯は数えない。見えている帯だけ）。
const KEY_SCREENS = [
  { name: '取引を登録', hash: '#/entry', ready: '.entry-actions-card', sizes: [[640, 450], [390, 844]], min: 5 },
  { name: '未確認', hash: '#/inbox', ready: '.inbox-bar', sizes: [[640, 450]], min: 5 },
  // 仕訳帳の本文の止まり場所は、期間・絞り込み・検索・表の行（行は1つだけ止まる）など。
  { name: '仕訳帳', hash: '#/journal', ready: '.journal-table tbody tr', sizes: [[640, 450]], min: 3 },
];
for (const s of KEY_SCREENS) for (const [width, height] of s.sizes) {
  test(`SC-02 幅${width}×${height}で、Tab と Shift+Tab で移った欄が、上下に固定した帯の裏に隠れない（${s.name}）`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await open(page, s.hash);
    await expect(page.locator(s.ready).first()).toBeVisible();
    await page.locator('main h1').focus();
    // フォーカスのある欄（ラジオなどは包むラベル）の四角と、上の帯の下の端・下に固定した帯の上の端。
    const where = () =>
      page.evaluate(() => {
        const el = document.activeElement;
        const box = (el.matches('input[type="radio"], input[type="checkbox"]') && el.closest('label')) || el;
        const r = box.getBoundingClientRect();
        const topbar = document.querySelector('.topbar').getBoundingClientRect().bottom;
        const bars = [...document.querySelectorAll('[data-bottom-bar], .bottomnav')].filter((b) => !b.contains(el) && b.getBoundingClientRect().height > 0).map((b) => b.getBoundingClientRect().top);
        return {
          inMain: Boolean(el.closest('main')),
          inBar: Boolean(el.closest('[data-bottom-bar]')),
          name: `${el.tagName.toLowerCase()} ${el.getAttribute('aria-label') || (el.labels && el.labels[0] ? el.labels[0].textContent.trim() : el.textContent.trim()).slice(0, 12)}`,
          top: r.top,
          bottom: r.bottom,
          topbar,
          floor: Math.min(...bars, window.innerHeight),
          visible: r.width > 0 && r.height > 0,
        };
      });
    const seen = [];
    for (let i = 0; i < 80; i += 1) {
      await page.keyboard.press('Tab');
      const w = await where();
      if (!w.inMain) break;
      if (!w.visible) continue;
      seen.push(w.name);
      expect(w.top, `Tab: ${w.name} が上の帯の裏にある`).toBeGreaterThanOrEqual(w.topbar - 0.5);
      if (!w.inBar) expect(w.bottom, `Tab: ${w.name} が下の帯の裏にある`).toBeLessThanOrEqual(w.floor + 0.5);
    }
    expect(seen.length, `本文の欄を Tab でたどれた（${seen.join('、')}）`).toBeGreaterThan(s.min);
    for (let i = 0; i < seen.length + 5; i += 1) {
      await page.keyboard.press('Shift+Tab');
      const w = await where();
      if (!w.inMain) break;
      if (!w.visible) continue;
      expect(w.top, `Shift+Tab: ${w.name} が上の帯（下の端 ${w.topbar}px）の裏にある`).toBeGreaterThanOrEqual(w.topbar - 0.5);
      if (!w.inBar) expect(w.bottom, `Shift+Tab: ${w.name} が下の帯の裏にある`).toBeLessThanOrEqual(w.floor + 0.5);
    }
  });
}

// 高さの低い画面（520px以下・幅767px以下）では、下のバーを隠し、全部の画面へは上の帯の「メニュー」から行く。
// 下に固定する帯（確定の帯）は、画面の下の端に付く。高い画面では、今までどおり下のバー（上の帯の「メニュー」は出さない）。
test('SC-03 高さ450では下のバーを隠し、上の帯の「メニュー」から全部の画面へ行ける。確定の帯は画面の下の端に付く', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 450 });
  await open(page, '#/inbox');
  await expect(page.locator('.inbox-bar')).toBeVisible();
  await expect(page.locator('.bottomnav')).toBeHidden();
  const menu = page.locator('.topbar').getByRole('button', { name: 'メニュー' });
  await expect(menu).toBeVisible();
  await expect(menu).toHaveAttribute('aria-haspopup', 'dialog');
  const box = await menu.boundingBox();
  expect(Math.min(box.width, box.height), '押せる大きさ（44px以上）').toBeGreaterThanOrEqual(44);
  // 確定の帯の下の端は、画面の下の端（下のバーの分の隙間を空けない）。
  await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
  await expect.poll(() => page.evaluate(() => Math.round(document.querySelector('.inbox-bar').getBoundingClientRect().bottom))).toBe(450);
  await menu.click();
  const drawer = page.getByRole('dialog', { name: 'メニュー' });
  await expect(drawer).toBeVisible();
  await drawer.getByRole('link', { name: '勘定科目' }).click();
  await expect(drawer).toHaveCount(0);
  await expect(page).toHaveURL(/#\/accounts$/);
  // 高い画面では、下のバーに戻り、上の帯の「メニュー」は出さない。
  await page.setViewportSize({ width: 640, height: 900 });
  await expect(page.locator('.bottomnav')).toBeVisible();
  await expect(menu).toBeHidden();
});

// SC-03 640×450 の未確認で、科目を打って Enter で次の行へ移ると、その行の店名と金額が、上の帯の下・確定の帯の上に見える
// （下のバーがあった頃は、金額が上の帯に半分隠れた）。科目は保存されるが、未確認のまま（確定はしない）。
test('SC-03 幅640×450の未確認で、科目を打って Enter で次の行へ移ると、その行の店名と金額が帯に隠れない', async ({ page }) => {
  await page.setViewportSize({ width: 640, height: 450 });
  await open(page, '#/inbox');
  const rows = page.locator('tr.inbox-row');
  await expect(rows.first()).toBeVisible();
  test.skip((await rows.count()) < 5, '未確認が少ないデータ');
  await rows.first().locator('td.c-account input').focus();
  for (let i = 1; i <= 3; i += 1) {
    await page.keyboard.insertText('つうしん');
    await page.keyboard.press('Enter');
    await expect(rows.nth(i).locator('td.c-account input')).toBeFocused();
    const m = await rows.nth(i).evaluate((tr) => {
      const r = (e) => e.getBoundingClientRect();
      return {
        desc: r(tr.querySelector('.desc-text')),
        amount: r(tr.querySelector('.amount')),
        topbar: r(document.querySelector('.topbar')).bottom,
        bar: r(document.querySelector('.inbox-bar')).top,
      };
    });
    for (const [what, b] of [['店名', m.desc], ['金額', m.amount]]) {
      expect(b.top, `${i + 1}行目の${what}が上の帯の裏`).toBeGreaterThanOrEqual(m.topbar);
      expect(b.bottom, `${i + 1}行目の${what}が確定の帯の裏`).toBeLessThanOrEqual(m.bar);
    }
  }
});

// サイドバーの下の「コマンド」「ショートカット」「表示」は、ほかの項目と同じ大きさと文字の色（控えめな色と小ささが重なっていた）。
// 768px の細い列の名前は 13px で、1行に収まる。
test('サイドバーの下の項目はほかの項目と同じ大きさと色。細い列の名前は13pxで1行に収まる', async ({ page }) => {
  await open(page, '#/');
  const style = (loc) => loc.evaluate((el) => ({ size: getComputedStyle(el).fontSize, color: getComputedStyle(el).color }));
  const main = await style(page.locator('.sidebar .nav-item[data-nav="journal"]'));
  const foot = await style(page.locator('.sidebar-foot [data-action="palette"]'));
  const text = await page.evaluate(() => getComputedStyle(document.body).color);
  expect(foot.size).toBe(main.size);
  expect(foot.color).toBe(text);
  await page.setViewportSize({ width: 800, height: 900 });
  await expect(page.locator('#app')).toHaveAttribute('data-rail', 'closed');
  const labels = await page.evaluate(() =>
    [...document.querySelectorAll('.sidebar .nav-item .nav-label, .sidebar .rail-toggle-label')]
      .filter((el) => el.getBoundingClientRect().width > 0)
      .map((el) => {
        const s = getComputedStyle(el);
        return { text: el.textContent.trim(), size: s.fontSize, oneLine: el.getBoundingClientRect().height <= parseFloat(s.fontSize) * 1.6, inside: el.getBoundingClientRect().width <= 64 };
      }),
  );
  expect(labels.length).toBeGreaterThan(10);
  for (const l of labels) {
    expect(l.size, l.text).toBe('13px');
    expect(l.oneLine, `${l.text} が2行に折れる`).toBe(true);
    expect(l.inside, `${l.text} が列の幅からはみ出す`).toBe(true);
  }
});

// 帳票のタブ（幅390）: 横に入りきらない間は、続きのある側の端をぼかす（次のタブの文字が途中で切れて見えるだけでは、気づきにくかった）。
// 端まで動かすと、その側のぼかしは消える。入りきる幅（1440）では、ぼかさない。
test('帳票のタブは、横に続きがある側の端をぼかし、入りきる幅ではぼかさない', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/reports/monthly');
  const tabs = page.getByRole('tablist', { name: '帳票の種類' });
  await expect(tabs).toBeVisible();
  await expect(tabs).toHaveAttribute('data-more', 'right');
  expect(await tabs.evaluate((el) => getComputedStyle(el).maskImage)).toContain('linear-gradient');
  await tabs.evaluate((el) => el.scrollBy(el.scrollWidth / 3, 0));
  await expect(tabs).toHaveAttribute('data-more', 'left right');
  await tabs.evaluate((el) => el.scrollTo(el.scrollWidth, 0));
  await expect(tabs).toHaveAttribute('data-more', 'left');
  // キーで端のタブへ移ると、選んだタブは、ぼかしの外（はっきり見える所）にある。
  await page.getByRole('tab', { name: '月次推移' }).focus();
  await expect(tabs).toHaveAttribute('data-more', 'right');
  await page.keyboard.press('ArrowLeft');
  const last = page.getByRole('tab', { selected: true });
  await expect(last).toBeFocused();
  const [t, l] = await Promise.all([tabs.boundingBox(), last.boundingBox()]);
  expect(l.x + l.width, '選んだタブの右の端が、ぼかしの内側').toBeLessThanOrEqual(t.x + t.width - (await tabs.getAttribute('data-more')).includes('right') * 40 + 1);
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(tabs).not.toHaveAttribute('data-more', /./);
  expect(await tabs.evaluate((el) => getComputedStyle(el).maskImage)).toBe('none');
});

// SC-02 幅390で、画面の下寄りの科目の欄を押すと、候補の一覧に6件以上が見え、欄は隠れない（下の余白が6件ぶんに足りなければ
// 上に開く。前は下に開き、画面の下端までの約5件しか見えなかった）。見えるかは、件の真ん中の点でいちばん手前がその件かで数える。
test('SC-02 幅390で、画面の下寄りの科目の欄を押すと、候補が6件以上見え、欄を隠さない', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  const input = page.locator('form.entry-form').getByRole('combobox', { name: /科目/ });
  await expect(input).toBeVisible();
  // 欄の上の端を、評価で見た位置（599px）の近くへ動かす。
  await input.evaluate((el) => window.scrollBy(0, el.getBoundingClientRect().top - 599));
  const top = (await input.boundingBox()).y;
  expect(top, '試験の前提: 欄が画面の下寄り（下に6件ぶんの余白が無い）').toBeGreaterThan(844 - 288 - 60);
  await input.click();
  const list = page.locator('.combo-list:not([hidden])');
  await expect(list).toBeVisible();
  const m = await list.evaluate((el, inputTop) => {
    const box = el.getBoundingClientRect();
    const seen = [...el.querySelectorAll('[role="option"]')].filter((o) => {
      const r = o.getBoundingClientRect();
      const y = r.top + r.height / 2;
      if (y < box.top || y > box.bottom || y < 0 || y > innerHeight) return false;
      const front = document.elementFromPoint(r.left + r.width / 2, y);
      return Boolean(front && o.contains(front));
    }).length;
    return { seen, listTop: box.top, listBottom: box.bottom, inputTop };
  }, top);
  expect(m.seen, '見える候補の数').toBeGreaterThanOrEqual(6);
  expect(m.listBottom, '一覧が欄を隠さない（上に開いた）').toBeLessThanOrEqual(m.inputTop);
  await page.keyboard.press('Escape');
  await expect(list).toHaveCount(0);
});

// SC-03 未確認の表で、下の行から1行ずつ上へフォーカスを戻すと、1行目の上に表の見出しが見えている（ページの scroll-padding-top。
// 余白が8pxだと、1行目の欄が画面の上の端に付き、見出しが画面の外に隠れた）。行は、画面の高さを超える数を作る。
// 遠くの欄へ一度に移ると、ブラウザは欄を真ん中へ寄せるので、↑ と同じく1行ずつ移る。
test('SC-03 未確認の表で、1行ずつ上へ戻って1行目に着くと、表の見出しが画面の中に見えている（幅1100）', async ({ page }) => {
  const N = 24;
  await page.route('**/api/v1/drafts?*', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    const base = body.items[0];
    body.items = Array.from({ length: N }, (_, i) => ({ ...base, id: 910000 + i, description: `見出しの確認 ${i + 1}`, amount: 1000 + i, direction: 'out' }));
    body.total = N;
    await route.fulfill({ response: res, json: body });
  });
  await page.setViewportSize({ width: 1100, height: 700 });
  await open(page, '#/inbox');
  const rows = page.locator('tr.inbox-row');
  await expect(rows).toHaveCount(N);
  await rows.last().locator('td.c-account input').focus();
  for (let i = N - 2; i >= 0; i -= 1) await rows.nth(i).locator('td.c-account input').focus();
  const m = await page.evaluate(() => {
    const head = document.querySelector('tr.inbox-row').closest('table').querySelector('thead').getBoundingClientRect();
    return { headTop: head.top, scrollY: window.scrollY };
  });
  expect(m.scrollY, '試験の前提: ページが下へ動いている').toBeGreaterThan(0);
  expect(m.headTop, `表の見出しの上の端 ${m.headTop}px`).toBeGreaterThanOrEqual(0);
});
