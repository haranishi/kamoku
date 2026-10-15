// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）のキー操作とフォーカスの直し（2026-10-15 の体験の評価への対応 g2）: 税区分の欄の上下の矢印、検索の欄の Esc、
// ⌘Z（Ctrl+Z）で無視を戻す、確定の後と「戻す」の後のフォーカスの行き先。
// 各試験は、自分の名前（uniqueText）を摘要に入れた行を取り込み、その名前で絞って使う（ほかの試験と混ざらない）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, importRows } from './helpers.mjs';

const rowsOf = (page) => page.locator('tr.inbox-row');
const accountOf = (row) => row.locator('td.c-account input[role="combobox"]');
const taxOf = (row) => row.locator('td.c-tax select');

/** n 行を取り込み、その名前で絞った未確認を開く。patch があれば、全部の行に入れる（科目など）。 */
async function prepare(page, prefix, n, patch = null, query = '') {
  await open(page, '#/');
  const token = uniqueText(prefix).replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  // 1月の日付にする（ほかの試験が10月に作る仕訳を、仕訳帳の1ページ目から押し出さないように）。
  const rows = Array.from({ length: n }, (_, i) => [`2026-01-${String((i % 28) + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(1000 + i * 10), '']);
  await importRows(page, card.code, rows);
  const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}&limit=100`)).json.items;
  if (patch) for (const d of items) await apiCall(page, 'PATCH', `/drafts/${d.id}`, patch);
  await open(page, `#/inbox?q=${encodeURIComponent(token)}${query}`);
  return { token, items };
}

test('SC-03 g2-2 税区分の欄の ↓ ↑ は値を切り替え、行は移らない（端で止まる）。科目の欄の ↓ は、今までどおり次の行へ', async ({ page }) => {
  const { token } = await prepare(page, 'E2E 税区分矢印', 2, { account: 'exp_supplies', tax_code: 'P10' });
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(2);
  const first = rows.nth(0);
  await taxOf(first).focus();
  await expect(taxOf(first)).toHaveValue('P10');
  // 支出の行の並び: 対象外・課税仕入10%・課税仕入8%軽減・…。先頭の隠した仮の値（科目で決まる）には戻らない。
  await page.keyboard.press('ArrowDown');
  await expect(taxOf(first)).toHaveValue('P8');
  await expect(taxOf(first)).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await page.keyboard.press('ArrowUp');
  await expect(taxOf(first)).toHaveValue('');
  await page.keyboard.press('ArrowUp');
  await expect(taxOf(first)).toHaveValue('');
  await expect(taxOf(first)).toBeFocused();
  await expect(taxOf(rows.nth(1))).toHaveValue('P10');
  // 矢印で変えた値は保存される（手で選んだ値として残る）。
  await expect.poll(async () => (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items.map((d) => d.tax_code)).toEqual([null, 'P10']);
  await expect(taxOf(first)).not.toHaveClass(/is-placeholder/);
  await accountOf(first).focus();
  await page.keyboard.press('ArrowDown');
  await expect(accountOf(rows.nth(1))).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(accountOf(first)).toBeFocused();
});

test('SC-03 g2-7 検索の欄: 欄にいる間は「Enter で探します」と出し、Esc で欄から抜ける（文字は残り、続けた g h でホームへ移る）', async ({ page }) => {
  await prepare(page, 'E2E 検索Esc', 1);
  const search = page.getByRole('searchbox', { name: '明細を文字で探す' });
  const hint = page.locator('.inbox-search .search-hint');
  await expect(hint).toBeHidden();
  await page.locator('main h1').focus();
  await page.keyboard.press('/');
  await expect(search).toBeFocused();
  await expect(hint).toBeVisible();
  await expect(hint).toHaveText('Enter で探します');
  await expect(search).toHaveAccessibleDescription('Enter で探します');
  await search.fill('E2E');
  await page.keyboard.press('Escape');
  await expect(search).not.toBeFocused();
  await expect(search).toHaveValue('E2E');
  await expect(hint).toBeHidden();
  await page.keyboard.press('g');
  await page.keyboard.press('h');
  await expect(page).toHaveURL(/#\/$/);
});

test('SC-03 g2-7 一覧で ⌘Z（Ctrl+Z）を押すと、最後に無視した行を戻し、その行へフォーカスする。文字の欄では、欄の取り消しに任せる', async ({ page }) => {
  const { token } = await prepare(page, 'E2E 元に戻す', 3);
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(3);
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await expect(rows).toHaveCount(2);
  const notice = page.locator('.toast').filter({ hasText: `「${token} 001」を無視しました。` });
  await expect(notice).toBeVisible();
  // メモの欄で打ってからの ⌘Z は、欄の文字の取り消しに任せる（画面は止めない＝defaultPrevented が偽。行は戻らない）。
  await page.evaluate(() => document.addEventListener('keydown', (e) => (e.key === 'z' || e.key === 'Z') && setTimeout(() => (window.__undoPrevented = e.defaultPrevented))));
  const memo = rows.nth(0).locator('td.c-memo input');
  await memo.click();
  await page.keyboard.type('abc');
  await page.keyboard.press('ControlOrMeta+z');
  await expect.poll(() => page.evaluate(() => window.__undoPrevented)).toBe(false);
  await page.waitForTimeout(300);
  await expect(rows).toHaveCount(2);
  await expect(page.locator('.toast').filter({ hasText: '未確認に戻しました' })).toHaveCount(0);
  // 行の印へ出てから ⌘Z。無視した行が戻り、その行の印にフォーカスが来る。古い「元に戻す」の通知は消える。
  await page.keyboard.press('Escape');
  await expect(rows.nth(0).locator('.cand-check')).toBeFocused();
  await page.keyboard.press('ControlOrMeta+z');
  await expect(page.locator('.toast').filter({ hasText: '未確認に戻しました' })).toBeVisible();
  await expect(rows).toHaveCount(3);
  await expect(rows.filter({ hasText: `${token} 001` }).locator('.cand-check')).toBeFocused();
  await expect(notice).toHaveCount(0);
  // 戻す行が無ければ、何もしない。
  await page.keyboard.press('ControlOrMeta+z');
  await expect(rows).toHaveCount(3);
});

/** 無視の送り（POST /drafts/{id}/ignore）の行の番号を、送った順に集める。 */
function watchIgnores(page) {
  const sent = [];
  page.on('request', (r) => {
    const m = r.method() === 'POST' && r.url().match(/\/drafts\/(\d+)\/ignore$/);
    if (m) sent.push(Number(m[1]));
  });
  return sent;
}

test('SC-03 r3-2 x を間を空けずに3回押すと、押した回数だけ上から順に無視し、同じ行を2回送らない', async ({ page }) => {
  // 前は応えを待つ間フォーカスが同じ行に残り、2回目・3回目の x が同じ行へ送り直されて、3回押しても1〜2件だった（E2 第2回）。
  const { token, items } = await prepare(page, 'E2E x連打', 5);
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(5);
  const sent = watchIgnores(page);
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await page.keyboard.press('x');
  await page.keyboard.press('x');
  await expect(rows).toHaveCount(2);
  await expect(rows.locator('.desc-text')).toHaveText([`${token} 004`, `${token} 005`]);
  await expect(rows.nth(0).locator('.cand-check')).toBeFocused();
  const ignored = (await apiCall(page, 'GET', `/drafts?status=ignored&q=${encodeURIComponent(token)}`)).json.items.map((d) => d.description).sort();
  expect(ignored).toEqual([`${token} 001`, `${token} 002`, `${token} 003`]);
  expect(sent).toEqual(items.slice(0, 3).map((d) => d.id));
  // 続けて無視しても、「元に戻す」の通知は1つだけ（最後の行のぶん）。積み重なると、表の右下の操作を覆った（E2 第3回）。
  const notices = page.locator('.toast').filter({ hasText: 'を無視しました' });
  await expect(notices).toHaveCount(1);
  await expect(notices).toContainText(`${token} 003`);
  // 3件とも ⌘Z で順に戻せる（新しい順）。
  for (const n of ['003', '002', '001']) {
    await page.keyboard.press('ControlOrMeta+z');
    await expect(rows.filter({ hasText: `${token} ${n}` })).toHaveCount(1);
  }
  await expect(rows).toHaveCount(5);
});

// 送りの順序そのものを確かめる（送り始めた順だけでは、前の応えを待たずに並べて送る作りでも通ってしまう。読み取りレビュー 2026-10-15）。
test('SC-03 r3-2 無視は、前の送りの応えが返るまで次を送らない（押した順に1件ずつ。保留を解くと次を送る）', async ({ page }) => {
  const { token, items } = await prepare(page, 'E2E 無視の直列', 3);
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(3);
  const sent = watchIgnores(page);
  let release;
  const gate = new Promise((resolve) => {
    release = resolve;
  });
  // 1件目の応えを、こちらが許すまで保留する。
  await page.route(
    `**/api/v1/drafts/${items[0].id}/ignore`,
    async (route) => {
      await gate;
      await route.continue();
    },
    { times: 1 },
  );
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await page.keyboard.press('x');
  // 2回目の x のぶんは、1件目の応えが返るまで送らない。送っている行は消えず、薄く出る。
  await expect.poll(() => sent.length).toBe(1);
  await page.waitForTimeout(400);
  expect(sent, '保留の間は、2件目を送らない').toEqual([items[0].id]);
  await expect(rows).toHaveCount(3);
  release();
  await expect(rows).toHaveCount(1);
  expect(sent, '保留を解くと、押した順に2件目を送る').toEqual([items[0].id, items[1].id]);
  await expect(rows.locator('.desc-text')).toHaveText([`${token} 003`]);
});

test('SC-03 r3-2 マウスで同じ行の「無視」を2回続けて押しても、送るのは1回（消えるのはその1行だけ）', async ({ page }) => {
  const { token, items } = await prepare(page, 'E2E 無視2回', 3);
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(3);
  const sent = watchIgnores(page);
  await rows.nth(0).getByRole('button', { name: /^無視/ }).dblclick();
  await expect(rows).toHaveCount(2);
  await expect(rows.locator('.desc-text')).toHaveText([`${token} 002`, `${token} 003`]);
  await page.waitForTimeout(300);
  expect(sent).toEqual([items[0].id]);
  await expect(page.locator('.toast').filter({ hasText: 'できません' })).toHaveCount(0);
});

test('SC-03 r3-2 続けて無視した途中の行が送れなかったら、その行を一覧に戻して理由を出す（前の行は無視したまま）', async ({ page }) => {
  const { token, items } = await prepare(page, 'E2E x失敗', 3);
  const rows = rowsOf(page);
  await expect(rows).toHaveCount(3);
  const second = items.find((d) => d.description === `${token} 002`);
  await page.route(`**/api/v1/drafts/${second.id}/ignore`, (route) => route.fulfill({ status: 422, json: { error: { code: 'bad_request', message: '試験の応え: 送れませんでした。' } } }), { times: 1 });
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await page.keyboard.press('x');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText(`${token} 002`);
  await expect(rows.nth(0)).not.toHaveClass(/is-leaving/);
  await expect(page.locator(`#draft-${second.id}-msg`)).toContainText('試験の応え: 送れませんでした。');
  // フォーカスは、2回目に押した時点で移った3行目のまま。
  await expect(rows.nth(1).locator('.cand-check')).toBeFocused();
  const ignored = (await apiCall(page, 'GET', `/drafts?status=ignored&q=${encodeURIComponent(token)}`)).json.items.map((d) => d.description);
  expect(ignored).toEqual([`${token} 001`]);
  // 戻った行は、もう一度 x で無視できる。
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await expect(rows).toHaveCount(1);
  // 最後の1行も無視すると、送り終えてから読み直し（空の状態）、フォーカスは見出しへ（body に落とさない）。
  await rows.nth(0).locator('.cand-check').focus();
  await page.keyboard.press('x');
  await expect(rows).toHaveCount(0);
  await expect(page.getByText('条件に合う明細はありません')).toBeVisible();
  await expect(page.locator('main h1')).toBeFocused();
});

for (const width of [1440, 390]) {
  test(`SC-03 g2-3 幅${width}: 確定の後、フォーカスのあった行が確定して消えたら、残った一覧の先頭の行の科目へ移り、その行が画面に入る`, async ({ page }) => {
    await page.setViewportSize({ width, height: width < 768 ? 844 : 900 });
    await prepare(page, `E2E 確定後${width}`, 30, { account: 'exp_supplies' });
    const rows = rowsOf(page);
    await expect(rows).toHaveCount(30);
    for (let i = 20; i < 25; i += 1) await rows.nth(i).locator('.cand-check').check();
    await accountOf(rows.nth(22)).focus();
    await page.keyboard.press('ControlOrMeta+Enter');
    await page.getByRole('dialog', { name: '5件を確定しますか' }).getByRole('button', { name: '確定する' }).click();
    await expect(page.locator('.toast').filter({ hasText: '5件を確定しました' })).toBeVisible();
    await expect(rows).toHaveCount(25);
    await expect(accountOf(rows.nth(0))).toBeFocused();
    // 先頭の行が、上の固定の帯（狭い幅）の下、下の固定の帯の上に見えている。行の位置は小数になるので、1pxの誤差は許す。
    const [box, top, bottom] = await Promise.all([
      rows.nth(0).boundingBox(),
      page.evaluate(() => {
        const bar = document.querySelector('.topbar');
        return bar && bar.getBoundingClientRect().height > 0 ? bar.getBoundingClientRect().bottom : 0;
      }),
      page.locator('.inbox-bar').evaluate((el) => el.getBoundingClientRect().top),
    ]);
    expect(box.y).toBeGreaterThanOrEqual(top - 1);
    expect(box.y + box.height).toBeLessThanOrEqual(bottom + 1);
  });
}

test('SC-03 g2-3 「無視した行」で「戻す」を押すと、次の行の「戻す」へ。次の行が無ければ、タブ「未確認」へ', async ({ page }) => {
  const { token, items } = await prepare(page, 'E2E 戻す後', 3);
  for (const d of items) await apiCall(page, 'POST', `/drafts/${d.id}/ignore`);
  await open(page, `#/inbox?status=ignored&q=${encodeURIComponent(token)}`);
  const buttons = page.locator('.ignored-table .restore-btn');
  await expect(buttons).toHaveCount(3);
  await buttons.nth(0).focus();
  await page.keyboard.press('Enter');
  await expect(buttons).toHaveCount(2);
  await expect(buttons.nth(0)).toBeFocused();
  await expect(buttons.nth(0)).toHaveAccessibleName(new RegExp(`${token} 002`));
  // 末尾の行を戻すと、次の行が無いので、タブ「未確認」へ。
  await buttons.nth(1).focus();
  await page.keyboard.press('Enter');
  await expect(buttons).toHaveCount(1);
  await expect(page.getByRole('tab', { name: /^未確認/ })).toBeFocused();
});
