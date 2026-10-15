// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）の確定の流れの守り: 送っている間は表を操作できない、確認を二重に開かない、応えが届かなくても一覧を合わせる、
// 行ごとの誤りを畳んだ欄の下に出す。カードの並びでは、Tab の順が見た目の順と同じ（「くわしく」を開いた後も）。
// 各試験は、自分の名前（uniqueText）を摘要に入れた行を取り込み、その名前で絞って使う（ほかの試験と混ざらない）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, importRows } from './helpers.mjs';

const rowsOf = (page) => page.locator('tr.inbox-row');
const accountOf = (row) => row.locator('td.c-account input[role="combobox"]');
const confirmDialog = (page, n) => page.getByRole('dialog', { name: `${n}件を確定しますか` });

/** 行を取り込み、科目を入れて（確定候補にできる形にして）、その名前で絞った未確認を開く。 */
async function prepare(page, prefix, n, { account = 'exp_supplies' } = {}) {
  await open(page, '#/');
  const token = uniqueText(prefix).replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  // 1月の日付にする（ほかの試験が10月に作る仕訳を、仕訳帳の1ページ目から押し出さないように）。
  const rows = Array.from({ length: n }, (_, i) => [`2026-01-${String(i + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(1000 + i * 10), '']);
  await importRows(page, card.code, rows);
  const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}&limit=50`)).json.items;
  if (account) for (const d of items) await apiCall(page, 'PATCH', `/drafts/${d.id}`, { account });
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(rowsOf(page)).toHaveCount(n);
  return { token, items };
}

/** 手で開ける門（route の応えを、試験が決めた時まで止める）。 */
function gate() {
  let open;
  const wait = new Promise((resolve) => {
    open = resolve;
  });
  return { wait, open };
}

test('SC-03 確定を送っている間は、表と絞り込みを操作できない（inert・aria-busy）。件数は固定し、終わったら元の行へ戻る', async ({ page }) => {
  await prepare(page, 'E2E 確定中', 3);
  const rows = rowsOf(page);
  await rows.nth(0).locator('.cand-check').check();
  await rows.nth(1).locator('.cand-check').check();
  const held = gate();
  await page.route('**/api/v1/drafts/confirm', async (route) => {
    await held.wait;
    await route.continue();
  });
  await accountOf(rows.nth(2)).focus();
  await page.keyboard.press('ControlOrMeta+Enter');
  await confirmDialog(page, 2).getByRole('button', { name: '確定する' }).click();
  const host = page.locator('.inbox-host');
  await expect(host).toHaveAttribute('aria-busy', 'true');
  expect(await page.evaluate(() => ['.inbox-host', '.inbox-tools'].map((s) => document.querySelector(s).inert))).toEqual([true, true]);
  const bar = page.locator('.inbox-bar');
  await expect(bar).toContainText('確定する行2件');
  await expect(bar).toContainText('確定しています');
  await expect(bar.getByRole('button', { name: /確定する/ })).toBeDisabled();
  // 送っている間に表を押しても、印は変わらない。Ctrl/Cmd+Enter でも、確認は開かない。
  const box = await rows.nth(2).locator('.cand-check').boundingBox();
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(rows.nth(2)).not.toHaveClass(/is-candidate/);
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(bar).toContainText('確定する行2件');
  held.open();
  await expect(page.locator('.toast').filter({ hasText: '2件を確定しました' })).toBeVisible();
  await expect(rows).toHaveCount(1);
  await expect(host).not.toHaveAttribute('aria-busy', /.*/);
  expect(await page.evaluate(() => document.querySelector('.inbox-host').inert)).toBe(false);
  // フォーカスは、確定の前にいた位置の行（3行目。残ったのは1行なので、その行）の科目へ戻る。
  await expect(accountOf(rows.nth(0))).toBeFocused();
});

test('SC-03 保存待ちから表・絞り込み・ページ送りを止め、確認でやめると解除して元の行へ戻る', async ({ page }) => {
  await prepare(page, 'E2E 保存待ち操作停止', 2);
  const rows = rowsOf(page);
  await rows.nth(0).locator('.cand-check').check();
  const held = gate();
  const arrived = gate();
  await page.route('**/api/v1/drafts/*', async (route) => {
    if (route.request().method() === 'PATCH') {
      arrived.open();
      await held.wait;
    }
    await route.continue();
  });
  const frozen = () => page.evaluate(() => ['.inbox-host', '.inbox-tools', '.pager'].map((s) => document.querySelector(s).inert));
  const host = page.locator('.inbox-host');
  const memo = rows.nth(0).locator('td.c-memo input');
  try {
    await memo.fill('保存待ちの操作停止');
    await page.keyboard.press('ControlOrMeta+Enter');
    await arrived.wait;
    await expect(host).toHaveAttribute('aria-busy', 'true');
    expect(await frozen()).toEqual([true, true, true]);
    await expect(page.locator('dialog[open]')).toHaveCount(0);
    const box = await rows.nth(1).locator('.cand-check').boundingBox();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await expect(rows.nth(1).locator('.cand-check')).not.toBeChecked();
    held.open();
    await expect(confirmDialog(page, 1)).toBeVisible();
    expect(await frozen()).toEqual([true, true, true]);
    await confirmDialog(page, 1).getByRole('button', { name: 'やめる' }).click();
    await expect(page.locator('dialog[open]')).toHaveCount(0);
    await expect(host).not.toHaveAttribute('aria-busy', /.*/);
    expect(await frozen()).toEqual([false, false, false]);
    await expect(memo).toBeFocused();
    await rows.nth(1).locator('.cand-check').check();
    await expect(rows.nth(1).locator('.cand-check')).toBeChecked();
  } finally {
    held.open();
  }
});

test('SC-03 保存を待つ間に Ctrl/Cmd+Enter を2回押しても、確認は1つしか開かない', async ({ page }) => {
  await prepare(page, 'E2E 確定二重', 2);
  const rows = rowsOf(page);
  await rows.nth(0).locator('.cand-check').check();
  const held = gate();
  await page.route('**/api/v1/drafts/*', async (route) => {
    if (route.request().method() === 'PATCH') await held.wait;
    await route.continue();
  });
  // メモを書いたまま確定へ。メモの保存（PATCH）の応えを止めている間に、もう一度押す。
  await rows.nth(0).locator('td.c-memo input').fill('二重の確認の試験');
  await page.keyboard.press('ControlOrMeta+Enter');
  await page.keyboard.press('ControlOrMeta+Enter');
  held.open();
  await expect(confirmDialog(page, 1)).toBeVisible();
  // 遅れて開く2つ目が無いことを、少し待ってから数える。
  await page.waitForTimeout(500);
  await expect(page.locator('dialog[open]')).toHaveCount(1);
  await confirmDialog(page, 1).getByRole('button', { name: 'やめる' }).click();
  await expect(page.locator('dialog[open]')).toHaveCount(0);
});

test('SC-03 確定の応えが届かなくても（通信の失敗）一覧を読み直す。もう一度確定すると、確定済みの行は候補から外れる', async ({ page }) => {
  const { token } = await prepare(page, 'E2E 確定通信', 2);
  const rows = rowsOf(page);
  await rows.nth(0).locator('.cand-check').check();
  await rows.nth(1).locator('.cand-check').check();
  // サーバーには届けて確定させ、画面には応えを返さない（通信が切れた）。
  await page.route('**/api/v1/drafts/confirm', async (route) => {
    await route.fetch();
    await route.abort('failed');
  }, { times: 1 });
  await page.keyboard.press('ControlOrMeta+Enter');
  await confirmDialog(page, 2).getByRole('button', { name: '確定する' }).click();
  await expect(page.locator('.toast').filter({ hasText: '一覧を読み込み直して' })).toBeVisible();
  await expect(rows).toHaveCount(0);
  expect((await apiCall(page, 'GET', `/drafts?status=confirmed&q=${encodeURIComponent(token)}`)).json.total).toBe(2);
  // 応えが届かなかったので、印は残る（確定できたかは分からない）。もう一度確定すると、確定済みとして外れる。
  const bar = page.locator('.inbox-bar');
  await expect(bar).toContainText('確定する行2件');
  await bar.getByRole('button', { name: /確定する/ }).click();
  await confirmDialog(page, 2).getByRole('button', { name: '確定する' }).click();
  await expect(page.locator('.toast').filter({ hasText: 'ほかの場所で確定済み' })).toBeVisible();
  await expect(bar).toContainText('確定する行0件');
});

test('SC-03 確定で返った行の誤りは、その欄（税区分）の誤りとして出し、狭い幅では畳んだ「くわしく」を開く', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const { items } = await prepare(page, 'E2E 確定誤り', 2);
  const rows = rowsOf(page);
  await rows.nth(0).locator('.cand-check').check();
  await expect(rows.nth(0).locator('.more-btn')).toHaveAttribute('aria-expanded', 'false');
  const message = '税区分を見直してください（試験の応え）。';
  await page.route('**/api/v1/drafts/confirm', (route) => route.fulfill({ json: { confirmed: [], failed: [{ id: items[0].id, error: { code: 'invalid', message, field: 'tax_code' } }] } }), { times: 1 });
  await page.keyboard.press('ControlOrMeta+Enter');
  await confirmDialog(page, 1).getByRole('button', { name: '確定する' }).click();
  await expect(page.locator('.toast').filter({ hasText: '1件は確定できませんでした' })).toBeVisible();
  await expect(rows.nth(0).locator('.more-btn')).toHaveAttribute('aria-expanded', 'true');
  await expect(rows.nth(0).locator('td.c-tax select')).toBeVisible();
  const msg = page.locator('.inbox-msg').first();
  await expect(msg).toContainText(message);
  await expect(msg.locator('[data-field="tax"]')).toHaveCount(1);
});

/** フォーカスを Tab で n 回進め、その都度の欄の名前と、ページの上からの位置を集める。 */
async function tabWalk(page, n) {
  const seq = [];
  for (let i = 0; i < n; i += 1) {
    seq.push(
      await page.evaluate(() => {
        const el = document.activeElement;
        const r = el.getBoundingClientRect();
        const tr = el.closest('tr.inbox-row');
        return { col: el.closest('td') ? el.closest('td').dataset.col : '', row: tr ? tr.dataset.id : '', y: Math.round(r.top + window.scrollY), x: Math.round(r.left) };
      }),
    );
    await page.keyboard.press('Tab');
  }
  return seq;
}

/** 見た目の読む順（上から下、同じ高さなら左から右）に進んでいるか。外れた所を返す。 */
function outOfOrder(seq) {
  const bad = [];
  for (let i = 1; i < seq.length; i += 1) {
    const [a, b] = [seq[i - 1], seq[i]];
    const below = b.y > a.y + 4;
    const sameLineRight = Math.abs(b.y - a.y) <= 4 && b.x > a.x;
    if (!below && !sameLineRight) bad.push(`${a.col}(${a.y},${a.x}) → ${b.col}(${b.y},${b.x})`);
  }
  return bad;
}

// 畳んだまま（候補から5回）と、「くわしく」を開いた後（くわしくから8回）の Tab の順。幅768は2段のカード（科目・くわしく・無視が
// 同じ段）。幅390は、2026-10-15 の第3回で「無視」を金額の下へ、「くわしく」を科目の横へ移した（1件が3段で高かった）ので、
// 見た目の順どおり 候補 → 無視 → 科目 → くわしく。どちらも、見た目の読む順と同じであること（outOfOrder）が本題。
const TAB_ORDER = {
  390: { folded: ['cand', 'ops', 'account', 'more', 'cand'], opened: ['more', 'date', 'tax', 'partner', 'memo', 'pct', 'cand', 'ops'] },
  768: { folded: ['cand', 'account', 'more', 'ops', 'cand'], opened: ['more', 'ops', 'date', 'tax', 'partner', 'memo', 'pct', 'cand'] },
};

for (const width of [390, 768]) {
  test(`SC-03 カードの並び（幅${width}）では、Tab の順が見た目の順と同じ。「くわしく」を開いた後も`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await prepare(page, `E2E 未確認Tab${width}`, 2);
    const first = rowsOf(page).nth(0);
    const id = await first.getAttribute('data-id');
    await first.locator('.cand-check').focus();
    const folded = await tabWalk(page, 5);
    expect(folded.map((s) => s.col)).toEqual(TAB_ORDER[width].folded);
    expect(folded[4].row).not.toBe(id);
    expect(outOfOrder(folded)).toEqual([]);
    await first.locator('.more-btn').focus();
    await page.keyboard.press('Enter');
    await expect(first.locator('.more-btn')).toHaveAttribute('aria-expanded', 'true');
    const opened = await tabWalk(page, 8);
    expect(opened.map((s) => s.col)).toEqual(TAB_ORDER[width].opened);
    expect(outOfOrder(opened)).toEqual([]);
  });
}

test('SC-03 幅を変えて表とカードを行き来すると、欄の並び（Tab の順）も表の順とカードの順を行き来する', async ({ page }) => {
  await prepare(page, 'E2E 未確認並び', 1);
  const order = () => page.evaluate(() => [...document.querySelector('tr.inbox-row').children].map((td) => td.dataset.col).join(','));
  const table = 'cand,date,desc,amount,account,more,tax,partner,memo,pct,ops';
  const cards = 'cand,desc,amount,account,more,ops,date,tax,partner,memo,pct';
  await expect.poll(order).toBe(table);
  await page.setViewportSize({ width: 768, height: 900 });
  await expect.poll(order).toBe(cards);
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect.poll(order).toBe(table);
});

// AC-11.8 下まで送ったときも、下の固定バーと下のバー（画面の端）の間に、本文の下の余白が出ない（隙間から地が見えた）。
for (const width of [390, 1440]) {
  test(`AC-11.8 未確認（幅${width}）: いちばん下まで送っても、固定バーの下に隙間が無い`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await prepare(page, `E2E 未確認下${width}`, 12, { account: null });
    await page.evaluate(() => window.scrollTo(0, document.documentElement.scrollHeight));
    const gap = await page.evaluate(() => {
      const r = document.querySelector('.inbox-bar').getBoundingClientRect();
      const nav = document.querySelector('.bottomnav');
      const nb = nav && nav.getBoundingClientRect().width > 0 ? nav.getBoundingClientRect() : null;
      return (nb ? nb.top : window.innerHeight) - r.bottom;
    });
    expect(Math.abs(gap)).toBeLessThan(1);
  });
}

for (const width of [390, 1440]) {
  test(`SC-03 幅${width}: 確定直後の通知が、下に付いた確定バーに重ならない（バーの上に出る）`, async ({ page }) => {
    await page.setViewportSize({ width, height: 844 });
    // 行が少なくて本文が画面より短いと、バーは本文の途中にあって下に付かない（そのとき通知はバーの下に出て、重ならない）。
    // 通知を持ち上げる動きを確かめるため、バーが下に付く行数にする。
    await prepare(page, `E2E 確定通知${width}`, 12);
    await rowsOf(page).first().locator('.cand-check').check();
    const bar = page.locator('.inbox-bar');
    await bar.getByRole('button', { name: /確定する/ }).click();
    const dlg = confirmDialog(page, 1);
    await expect(dlg.locator('.phrase .nw').filter({ hasText: '変えられません。' })).toHaveCount(1);
    await dlg.getByRole('button', { name: '確定する' }).click();
    const notice = page.locator('.toast').filter({ hasText: '1件を確定しました' }).last();
    await expect(notice).toBeVisible();
    // バーが、画面の下（幅767px以下は下のバーの上）に付いている。
    const gap = await page.evaluate(() => {
      const r = document.querySelector('.inbox-bar').getBoundingClientRect();
      const nav = document.querySelector('.bottomnav');
      const nb = nav && nav.getBoundingClientRect().width > 0 ? nav.getBoundingClientRect() : null;
      return (nb ? nb.top : window.innerHeight) - r.bottom;
    });
    expect(Math.abs(gap)).toBeLessThan(1);
    // 持ち上げの計算（次の描画の枠）が済むのを待って、通知がバーの上に出ていて、重ならないことを確かめる。
    await expect
      .poll(async () => {
        const [a, b] = await Promise.all([notice.boundingBox(), bar.boundingBox()]);
        if (!a || !b) return 'no-box';
        const overlaps = a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y;
        return overlaps ? 'overlap' : a.y + a.height <= b.y ? 'above' : 'below';
      })
      .toBe('above');
  });
}
