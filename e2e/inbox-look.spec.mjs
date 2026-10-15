// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）の見た目の直し（2026-10-15 の第3回。見た目の採点 第2回と E4 第2回への対応 r3-7）: 畳んだカードの高さ、
// 必須の科目の示し方、行の下の文の置き場所と見出し、カードの「確定する」の札と金額の単位、高さの低い画面の確定の帯。
// 各試験は、自分の名前（uniqueText）を摘要に入れた行を取り込み、その名前で絞って使う（ほかの試験と混ざらない）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, newWallet, importRows, withAccountDefaults } from './helpers.mjs';

const rowsOf = (page) => page.locator('tr.inbox-row');
const accountOf = (row) => row.locator('td.c-account input[role="combobox"]');

/** n 行を取り込み（1月の日付）、patch があれば全部の行に入れ、その名前で絞った未確認を開く。 */
async function prepare(page, prefix, n, patch = null) {
  await open(page, '#/');
  const token = uniqueText(prefix).replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  const rows = Array.from({ length: n }, (_, i) => [`2026-01-${String((i % 28) + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(1000 + i * 10), '']);
  await importRows(page, card.code, rows);
  const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}&limit=100`)).json.items;
  if (patch) for (const d of items) await apiCall(page, 'PATCH', `/drafts/${d.id}`, patch);
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(rowsOf(page)).toHaveCount(n);
  return { token, items };
}

/**
 * 試験の最後に、その口座の未確認の行を無視して片付ける。通し試験は1つの帳簿を全部の試験で使い、未確認の数を前提にする試験
 * （reports.spec の「500件以下」）があるため、行を多く作る試験は残さない。
 */
async function ignoreAll(page, wallet) {
  const items = (await apiCall(page, 'GET', `/drafts?status=draft&wallet=${wallet}&limit=100`)).json.items;
  for (const d of items) await apiCall(page, 'POST', `/drafts/${d.id}/ignore`);
}

/** 畳んだカード（行の下の文が無い行）の高さ。 */
const rowHeights = (page) => page.evaluate(() => [...document.querySelectorAll('tr.inbox-row')].map((tr) => Math.round(tr.getBoundingClientRect().height)));

// 前（第2回）の高さ: 幅390で、科目のある行173px・科目の無い行199px。幅768で、121px・147px（25件で5,650px・4,180px。
// 25件のうち22件が科目の無い行）。目標は一覧が各20%以上短くなること。上限は、前の科目の無い行の8割（390は前の科目のある行の8割）。
// 科目の無い行に1段（札）を足さないので、科目のある行と無い行は同じ高さになる。
for (const [width, limit] of [[390, 138], [768, 117]]) {
  test(`SC-03 r3-7 幅${width}: 畳んだカードは、科目のある行も無い行も同じ高さで、${limit}px以下（前の2割減）`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    // 採点の画面と同じく、短い摘要の行で見る（長い摘要は2行に折れて高くなる。それは中身による高さ）。新しい口座で絞る。
    await open(page, '#/');
    const w = await newWallet(page, 'card', uniqueText('E2E 高さ'));
    await importRows(page, w.code, ['サンプルストア', 'デモ書店', 'サンプル交通', 'デモ電器'].map((d, i) => [`2026-01-0${i + 1}`, d, String(1000 + i), '']));
    await open(page, `#/inbox?wallet=${w.code}`);
    await expect(rowsOf(page)).toHaveCount(4);
    await rowsOf(page).nth(0).locator('td.c-account input').click();
    await page.keyboard.insertText('しょうもう');
    await page.keyboard.press('Enter');
    await expect(rowsOf(page).nth(0)).toHaveClass(/is-candidate/);
    const heights = await rowHeights(page);
    expect(Math.max(...heights), `行の高さ ${heights.join(',')}`).toBeLessThanOrEqual(limit);
    expect(Math.max(...heights) - Math.min(...heights), `行の高さ ${heights.join(',')}`).toBeLessThanOrEqual(2);
    await ignoreAll(page, w.code);
  });
}

test('SC-03 r3-7 科目が空の行は、科目の欄の「科目（必須）」で示す（任意の取引先・メモと文字で見分けられる）。読み上げにも必須を伝える', async ({ page }) => {
  await prepare(page, 'E2E 必須', 1);
  const row = rowsOf(page).nth(0);
  await expect(accountOf(row)).toHaveAttribute('placeholder', '科目（必須）');
  await expect(accountOf(row)).toHaveAttribute('aria-required', 'true');
  await expect(row.locator('td.c-partner input')).toHaveAttribute('placeholder', '取引先');
  await expect(row.locator('td.c-memo input')).toHaveAttribute('placeholder', 'メモ');
  await expect(row.locator('td.c-partner input')).not.toHaveAttribute('aria-required', 'true');
  // 摘要の上の「科目なし」の札は出さない（同じことを科目の欄が言う。25行に並ぶ札が、摘要と金額より先に目に入った）。
  await expect(row.locator('td.c-desc')).not.toContainText('科目なし');
});

test('SC-03 r3-7 幅1440: 行の下の文は、その欄の側に寄せる（税区分の話は税区分の列の下、日付の話は左）。要点を見出しにし、続きは小さく', async ({ page }) => {
  await open(page, '#/');
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 100 }, async () => {
    await prepare(page, 'E2E 文の場所', 2, { account: 'exp_comm', tax_code: 'P10X' });
    const msg = page.locator('tr.inbox-msg').first();
    const tax = msg.locator('[data-for="tax"]');
    const date = msg.locator('[data-for="date"]');
    await expect(tax).toHaveText('税区分が、科目の既定（課税仕入10%）と違います。');
    await expect(date.locator('.note-head')).toHaveText('日付は、課税仕入れの日にしてください。');
    await expect(date.locator('.note-detail')).toHaveText('商品の引渡し・役務の提供が完了した日です。経過措置の割合は、この行の日付で決まります。');
    const [taxBox, dateBox, taxHead, dateHead] = await Promise.all([tax.boundingBox(), date.boundingBox(), page.locator('th.c-tax').boundingBox(), page.locator('th.c-date').boundingBox()]);
    expect(Math.abs(taxBox.x - taxHead.x)).toBeLessThanOrEqual(4);
    expect(dateBox.x).toBeLessThan(taxHead.x);
    expect(Math.abs(dateBox.x - dateHead.x)).toBeLessThanOrEqual(12);
    // 2つの文は、同じ段に左右に並ぶ（1つの塊に見えない）。
    expect(Math.abs(taxBox.y - dateBox.y)).toBeLessThanOrEqual(4);
    expect(dateBox.x + dateBox.width).toBeLessThanOrEqual(taxBox.x + 1);
  });
});

for (const width of [390, 768]) {
  test(`SC-03 r3-7 幅${width}: 行の下の文は、カードの科目の欄のすぐ下に出る（くわしく・無視の段の下ではない）`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await open(page, '#/');
    await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 100 }, async () => {
      await prepare(page, `E2E 文カード${width}`, 2, { account: 'exp_comm', tax_code: 'P8' });
      const row = rowsOf(page).nth(0);
      const note = page.locator('tr.inbox-msg').first().locator('[data-for="tax"]');
      await expect(note).toBeVisible();
      const [field, ops, more, box] = await Promise.all([accountOf(row).boundingBox(), row.locator('.ignore-btn').boundingBox(), row.locator('.more-btn').boundingBox(), note.boundingBox()]);
      // 文の上の端は、科目の欄の下の端から少しだけ下（間に、くわしく・無視の段が無い）。
      expect(box.y).toBeGreaterThanOrEqual(field.y + field.height - 1);
      expect(box.y - (field.y + field.height)).toBeLessThanOrEqual(16);
      expect(box.y).toBeGreaterThanOrEqual(Math.max(ops.y + ops.height, more.y + more.height) - 1);
      // 文の左の端は、科目の欄の左の端にそろう。
      expect(Math.abs(box.x - field.x)).toBeLessThanOrEqual(4);
    });
  });
}

test('SC-03 r3-7 幅390: 確定する行には、本文の側に「確定する」の札が出る（チェックの下の小さな字ではない）。金額に単位が付く。幅1440では札も単位も出さない', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await prepare(page, 'E2E 札', 2, { account: 'exp_supplies' });
  const [first, second] = [rowsOf(page).nth(0), rowsOf(page).nth(1)];
  await first.locator('.cand-check').check();
  const badge = first.locator('.cand-badge');
  await expect(badge).toBeVisible();
  await expect(badge).toHaveText('確定する');
  await expect(second.locator('.cand-badge')).toBeHidden();
  // 札は、チェックの右（本文の側）にあり、読める大きさ（12px以上）。読み上げは、チェックの名前と状態に任せる。
  const [check, b] = await Promise.all([first.locator('.cand-check').boundingBox(), badge.boundingBox()]);
  expect(b.x).toBeGreaterThan(check.x + check.width);
  expect(Number.parseFloat(await badge.evaluate((el) => getComputedStyle(el).fontSize))).toBeGreaterThanOrEqual(12);
  await expect(badge).toHaveAttribute('aria-hidden', 'true');
  // チェックの下の小さな字（.cand-text）は、読み上げ用に1pxへ切り詰めたまま（見た目には出さない）。
  const under = await first.locator('.cand-text').boundingBox();
  expect(under.width).toBeLessThanOrEqual(1);
  await expect(first.locator('.amount')).toHaveText(/^-1,000円$/);
  await page.setViewportSize({ width: 1440, height: 900 });
  await expect(badge).toBeHidden();
  await expect(first.locator('.amount-unit')).toBeHidden();
  await expect(page.locator('.inbox-table th.c-amount')).toHaveText('金額（円）');
});

for (const [width, height] of [[640, 450], [844, 390]]) {
  test(`SC-03 r3-7 高さの低い画面（${width}×${height}）: 確定の帯は1行（約56px）。Enter で次の行へ移っても、その行の摘要と金額が上の帯に隠れない`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    // 摘要は2行に折れる長さにする（科目の欄だけが見える位置で止まると、その上の2行が上の帯の裏に入る。1行だと、余白が無くても
    // 数px差で見えてしまい、確かめにならない）。新しい口座で絞る。
    await open(page, '#/');
    const w = await newWallet(page, 'card', uniqueText('E2E 低い画面'));
    const long = (i) => `サンプル商事株式会社 九月分のご利用料金 振込手数料差引後のお支払い分 ほか${i + 1}件（明細の写し）`;
    await importRows(page, w.code, Array.from({ length: 8 }, (_, i) => [`2026-01-${String(i + 1).padStart(2, '0')}`, long(i), String(1000 + i), '']));
    await open(page, `#/inbox?wallet=${w.code}`);
    await expect(rowsOf(page)).toHaveCount(8);
    for (const d of (await apiCall(page, 'GET', `/drafts?wallet=${w.code}&limit=20`)).json.items) await apiCall(page, 'PATCH', `/drafts/${d.id}`, { account: 'exp_supplies' });
    await page.reload();
    await expect(rowsOf(page).nth(0).locator('td.c-account input')).toHaveValue('消耗品費');
    expect((await rowsOf(page).nth(0).locator('.desc-text').boundingBox()).height, '試験の前提: 摘要が2行に折れている').toBeGreaterThan(30);
    const bar = page.locator('.inbox-bar');
    // 印が0件の帯は、案内の1行だけ。
    expect((await bar.boundingBox()).height).toBeLessThanOrEqual(48);
    await expect(bar.locator('.inbox-bar-main')).toBeHidden();
    const rows = rowsOf(page);
    const where = () =>
      page.evaluate(() => {
        const el = document.activeElement;
        const tr = el.closest('tr.inbox-row');
        const top = document.querySelector('.topbar');
        const topEdge = top && top.getBoundingClientRect().height ? top.getBoundingClientRect().bottom : 0;
        const b = document.querySelector('.inbox-bar').getBoundingClientRect();
        return { desc: tr.querySelector('.desc-text').getBoundingClientRect().top, amount: tr.querySelector('.amount').getBoundingClientRect().top, field: el.getBoundingClientRect().bottom, topEdge, barTop: b.top, barH: b.height };
      });
    const seen = (m, label) => {
      expect(m.desc, `${label}の摘要が上の帯の裏にある`).toBeGreaterThanOrEqual(m.topEdge - 0.5);
      expect(m.amount, `${label}の金額が上の帯の裏にある`).toBeGreaterThanOrEqual(m.topEdge - 0.5);
      expect(m.field, `${label}の科目の欄が確定の帯の裏にある`).toBeLessThanOrEqual(m.barTop + 0.5);
      expect(m.barH).toBeLessThanOrEqual(64);
    };
    await accountOf(rows.nth(0)).focus();
    for (let i = 0; i < 6; i += 1) {
      await page.keyboard.press('Enter');
      await expect(accountOf(rows.nth(i + 1))).toBeFocused();
      seen(await where(), `Enter: ${i + 2}行目`);
    }
    // ↑ で1行ずつ上の行へ戻っても、戻った行の摘要と金額（科目の欄の上の段）が上の帯に隠れない（欄だけが見える位置で止まった）。
    for (let i = 6; i > 0; i -= 1) {
      await page.keyboard.press('ArrowUp');
      await expect(accountOf(rows.nth(i - 1))).toBeFocused();
      seen(await where(), `↑: ${i}行目`);
    }
    // 件数・合計と「確定する」が、同じ1行に並ぶ。
    const [sums, button] = await Promise.all([bar.locator('.inbox-sums').boundingBox(), bar.getByRole('button', { name: /確定する/ }).boundingBox()]);
    expect(button.y).toBeLessThan(sums.y + sums.height);
    expect(button.x).toBeGreaterThan(sums.x + sums.width - 1);
    await expect(bar).toContainText('確定する行6件');
    await ignoreAll(page, w.code);
  });
}
