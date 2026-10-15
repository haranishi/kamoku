// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）。キーボードだけで科目を選びながら確定候補にして確定する。日本語入力の変換中のEnter、読みでの絞り込み、
// 科目が空の行の拒否、取り込まない／戻す、上下の矢印、既定値の決まり、経過措置の案内、ほかの場所での確定、免税。
// 各試験は、自分の名前（uniqueText）を摘要に入れた行を取り込んで作り、その名前で絞って使う（ほかの試験と混ざらない）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, withSettings, walletOf, importRows } from './helpers.mjs';

const rowsOf = (page) => page.locator('tr.inbox-row');
const accountOf = (row) => row.locator('td.c-account input[role="combobox"]');

async function prepare(page, prefix, n, amount = (i) => 1100 + i * 110) {
  await open(page, '#/');
  const token = uniqueText(prefix).replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  // 1月の日付にする（ほかの試験が10月に作る仕訳を、仕訳帳の1ページ目から押し出さないように）。
  const rows = Array.from({ length: n }, (_, i) => [`2026-01-${String((i % 9) + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(amount(i)), '']);
  await importRows(page, card.code, rows);
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(rowsOf(page)).toHaveCount(n);
  return token;
}

test('AC-05.3 キーボードだけで、科目を選びながら10行を確定候補にして、Ctrl/Cmd+Enterでまとめて確定する', async ({ page }) => {
  const token = await prepare(page, 'E2E 未確認10', 10);
  // 見出し → 検索欄（/）→ Tab 4回で、1行目の科目の欄へ（見出しの「すべて選ぶ」、候補の欄、日付の欄の次）。
  // 「すべて選ぶ」を見出しに足したので、2026-10-14 の画面の直し（F2）で3回から4回にした。
  await page.keyboard.press('/');
  await expect(page.getByRole('searchbox', { name: '明細を文字で探す' })).toBeFocused();
  for (let i = 0; i < 4; i += 1) await page.keyboard.press('Tab');
  await expect(accountOf(rowsOf(page).nth(0))).toBeFocused();
  for (let i = 0; i < 10; i += 1) {
    await page.keyboard.insertText('しょうもう');
    await expect(page.getByRole('option', { name: /消耗品費/ }).first()).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(rowsOf(page).nth(i)).toHaveClass(/is-candidate/);
    await expect(accountOf(rowsOf(page).nth(i))).toHaveValue('消耗品費');
  }
  // 最後の行の後は、確定のボタンへ移る。下のバーに件数と合計が出る。
  await expect(page.getByRole('button', { name: /確定する/ })).toBeFocused();
  const bar = page.locator('.inbox-bar');
  const sum = Array.from({ length: 10 }, (_, i) => 1100 + i * 110).reduce((a, b) => a + b, 0);
  await expect(bar).toContainText('確定する行10件');
  await expect(bar).toContainText(`出金${sum.toLocaleString('en-US')}円`);
  await page.keyboard.press('ControlOrMeta+Enter');
  const dialog = page.getByRole('dialog', { name: '10件を確定しますか' });
  await expect(dialog).toContainText(`出金の合計${sum.toLocaleString('en-US')}円`);
  await expect(dialog).toContainText('入金の合計0円');
  await expect(dialog.getByRole('button', { name: '確定する' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.locator('.toast').filter({ hasText: '10件を確定しました' })).toBeVisible();
  await expect(rowsOf(page)).toHaveCount(0);
  const drafts = (await apiCall(page, 'GET', `/drafts?status=confirmed&q=${encodeURIComponent(token)}&limit=50`)).json;
  expect(drafts.total).toBe(10);
  const entries = (await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(token)}&state=all&limit=50`)).json;
  expect(entries.total).toBe(10);
  expect(entries.items.every((e) => e.lines.some((l) => l.account === 'exp_supplies' && l.side === 'debit'))).toBe(true);
});

test('AC-05.2 変換中のEnterでは、候補の選択にも確定候補にも働かない。変換を確定してからのEnterで1回で進む', async ({ page }) => {
  await prepare(page, 'E2E 未確認IME', 2);
  const first = rowsOf(page).nth(0);
  await accountOf(first).click();
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'しょうもう', selectionStart: 5, selectionEnd: 5 });
  // 変換中の文字でも絞る（AC-05.1）。
  await expect(page.getByRole('option', { name: /消耗品費/ }).first()).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(accountOf(first)).toBeFocused();
  await expect(accountOf(first)).toHaveValue('しょうもう');
  await expect(first).not.toHaveClass(/is-candidate/);
  // 変換中のEnterは、何も起こさない（理由の表示も出ない）。
  await expect(page.locator('.inbox-msg').first()).toBeHidden();
  await cdp.send('Input.insertText', { text: 'しょうもう' });
  await page.waitForTimeout(120);
  await page.keyboard.press('Enter');
  await expect(first).toHaveClass(/is-candidate/);
  await expect(accountOf(first)).toHaveValue('消耗品費');
  await expect(accountOf(rowsOf(page).nth(1))).toBeFocused();
});

test('AC-05.1 ひらがなの読みで科目が絞れる。科目が空の行はEnterで「科目を選んでください」と出して動かない', async ({ page }) => {
  await prepare(page, 'E2E 未確認読み', 2);
  const first = rowsOf(page).nth(0);
  await accountOf(first).click();
  await page.keyboard.insertText('つうしん');
  await expect(page.getByRole('option', { name: /通信費/ })).toBeVisible();
  await expect(page.getByRole('option', { name: /消耗品費/ })).toHaveCount(0);
  await page.keyboard.press('Escape');
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.press('Backspace');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Enter');
  await expect(page.locator('.inbox-msg').first()).toContainText('科目を選んでください');
  await expect(accountOf(first)).toBeFocused();
  await expect(first).not.toHaveClass(/is-candidate/);
  // 確定候補が無いまま確定しようとすると、理由を出す。
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.locator('.inbox-note')).toContainText('確定する行がありません');
  // 帯の文は最初から同じ文で、押しても変わらず、読み上げでは無音だった。通知（読み上げの領域）でも返す（体験の評価 第3回 E4）。
  await expect(page.locator('.toast').filter({ hasText: '確定する行がありません' })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('AC-05.7 x で無視すると一覧から消え、「無視した行」から戻せる', async ({ page }) => {
  const token = await prepare(page, 'E2E 未確認無視', 2);
  const first = rowsOf(page).nth(0);
  await accountOf(first).click();
  // 1回目の Esc は候補の一覧を閉じるだけ。2回目の Esc で行の印（候補の欄）へ。そこで x。
  await page.keyboard.press('Escape');
  await expect(accountOf(first)).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(first.locator('.cand-check')).toBeFocused();
  await page.keyboard.press('x');
  await expect(page.locator('.toast').filter({ hasText: `「${token} 001」を無視しました。` })).toBeVisible();
  await expect(rowsOf(page)).toHaveCount(1);
  await expect(rowsOf(page).nth(0).locator('.cand-check')).toBeFocused();
  await page.getByRole('tab', { name: /無視した行/ }).click();
  await expect(page).toHaveURL(/status=ignored/);
  const ignored = page.locator('.ignored-table tbody tr');
  await expect(ignored).toHaveCount(1);
  await expect(ignored.first()).toContainText(`${token} 001`);
  await ignored.first().getByRole('button', { name: /戻す/ }).click();
  await expect(page.locator('.toast').filter({ hasText: '未確認に戻しました' })).toBeVisible();
  await expect(ignored).toHaveCount(0);
  await page.getByRole('tab', { name: /未確認/ }).click();
  await expect(rowsOf(page)).toHaveCount(2);
});

test('SC-03 上下の矢印は同じ列の隣の行へ。Shift+Space で確定候補の印を付け外し、印は絞り込みを変えても残る', async ({ page }) => {
  const token = await prepare(page, 'E2E 未確認矢印', 3);
  const rows = rowsOf(page);
  await accountOf(rows.nth(0)).click();
  await page.keyboard.press('Escape');
  await page.keyboard.press('ArrowDown');
  await expect(accountOf(rows.nth(1))).toBeFocused();
  await page.keyboard.press('ArrowUp');
  await expect(accountOf(rows.nth(0))).toBeFocused();
  // 科目が空の行には、印を付けられない。
  await page.keyboard.press('Shift+Space');
  await expect(rows.nth(0)).not.toHaveClass(/is-candidate/);
  await expect(page.locator('.inbox-msg').first()).toContainText('科目を選んでください');
  await page.keyboard.insertText('しょうもう');
  await page.keyboard.press('Tab');
  await accountOf(rows.nth(0)).focus();
  await page.keyboard.press('Shift+Space');
  await expect(rows.nth(0)).toHaveClass(/is-candidate/);
  await page.keyboard.press('Shift+Space');
  await expect(rows.nth(0)).not.toHaveClass(/is-candidate/);
  await page.keyboard.press('Enter');
  await expect(rows.nth(0)).toHaveClass(/is-candidate/);
  // 絞り込みを変えて戻っても、印は残る（画面の中の集合）。
  await page.goto(`/#/inbox?q=${encodeURIComponent(`${token} 002`)}`);
  await expect(rows).toHaveCount(1);
  await expect(page.locator('.inbox-bar')).toContainText('確定する行1件');
  await page.goBack();
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toHaveClass(/is-candidate/);
  await expect(rows.nth(0).locator('.cand-check')).toBeChecked();
});

test('api-inbox の決まり: 科目を変えたとき、いまの値が前の科目の既定値と同じなら新しい既定値、違えば残す', async ({ page }) => {
  await open(page, '#/');
  const before = Object.fromEntries((await apiCall(page, 'GET', '/accounts?include_disabled=1')).json.items.filter((a) => ['exp_comm', 'exp_vehicle', 'exp_rent'].includes(a.code)).map((a) => [a.code, a]));
  const set = (code, p) => apiCall(page, 'PATCH', `/accounts/${code}`, p);
  await set('exp_comm', { default_tax: 'P10', default_pct: 70 });
  await set('exp_vehicle', { default_tax: 'P10', default_pct: 30 });
  await set('exp_rent', { default_tax: null, default_pct: 100 });
  try {
    const token = await prepare(page, 'E2E 未確認既定', 1);
    const row = rowsOf(page).nth(0);
    const pick = async (reading, name) => {
      await accountOf(row).click();
      await page.keyboard.press('ControlOrMeta+a');
      await page.keyboard.insertText(reading);
      await page.getByRole('option', { name: new RegExp(name) }).first().click();
    };
    await pick('つうしん', '通信費');
    await expect(row.locator('td.c-tax select')).toHaveValue('P10');
    await expect(row.locator('td.c-pct input')).toHaveValue('70');
    await row.locator('td.c-pct input').fill('50');
    await row.locator('td.c-pct input').press('Tab');
    await pick('しゃりょう', '車両費');
    await expect(row.locator('td.c-pct input')).toHaveValue('50');
    await expect(row.locator('td.c-tax select')).toHaveValue('P10');
    await pick('ちだい', '地代家賃');
    // 既定の税区分が無い科目で、手で選んでいない税区分は、仮の値「科目で決まる」（2026-10-15 の体験の評価の直し g2 の1）。
    await expect(row.locator('td.c-tax select')).toHaveValue('__unset');
    await expect(row.locator('td.c-pct input')).toHaveValue('50');
    await expect.poll(async () => {
      const d = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items[0];
      return [d.account, d.tax_code, d.business_pct];
    }).toEqual(['exp_rent', null, 50]);
  } finally {
    for (const [code, a] of Object.entries(before)) await set(code, { default_tax: a.default_tax, default_pct: a.default_pct });
  }
});

test('SC-03 日付を直せて、税区分が経過措置（P10X）の行には「課税仕入れの日にしてください」と出す', async ({ page }) => {
  const token = await prepare(page, 'E2E 未確認日付', 1);
  const row = rowsOf(page).nth(0);
  const date = row.locator('td.c-date input');
  await date.click();
  await date.fill('0107');
  await date.press('Tab');
  // 未確認の日付は、ほかの画面と同じく曜日つき（今年なら年を省く。2026-10-15 の P3 の直し）。
  await expect(date).toHaveValue(/01\/07 \(.\)$/);
  await expect.poll(async () => (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items[0].date).toMatch(/-01-07$/);
  await accountOf(row).click();
  await page.keyboard.insertText('がいちゅう');
  await page.keyboard.press('Enter');
  await row.locator('td.c-tax select').selectOption('P10X');
  await expect(page.locator('.inbox-msg').first()).toContainText('課税仕入れの日にしてください');
  await row.locator('td.c-tax select').selectOption('P10');
  await expect(page.locator('.inbox-msg').first()).toBeHidden();
});

test('409 already_confirmed: ほかの場所で確定された行を直すと「他の場所で更新されました」と出して読み込み直す', async ({ page }) => {
  const token = await prepare(page, 'E2E 未確認競合', 2);
  const d = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items[0];
  await apiCall(page, 'PATCH', `/drafts/${d.id}`, { account: 'exp_supplies' });
  const res = await apiCall(page, 'POST', '/drafts/confirm', { ids: [d.id] });
  expect(res.json.confirmed).toHaveLength(1);
  const memo = rowsOf(page).nth(0).locator('td.c-memo input');
  await memo.fill('画面で書いたメモ');
  await memo.press('Tab');
  await expect(page.locator('.toast').filter({ hasText: '他の場所で更新されました。読み込み直します。' })).toBeVisible();
  await expect(rowsOf(page)).toHaveCount(1);
});

test('免税の設定では、税区分の列を隠し、科目の既定の税区分を入れない（空で送る）', async ({ page }) => {
  await open(page, '#/');
  await withSettings(page, { tax_mode: 'exempt', invoice_number: null }, async () => {
    const token = await prepare(page, 'E2E 未確認免税', 2);
    // 2行目は、課税の設定だったときに税区分まで決めてあった行（確定候補にするときに空へ戻すこと）。
    const second = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items.find((d) => d.description.endsWith('002'));
    await apiCall(page, 'PATCH', `/drafts/${second.id}`, { account: 'exp_comm', tax_code: 'P10' });
    await page.reload();
    await expect(page.locator('th.c-tax')).toBeHidden();
    // 画面から送った保存に、税区分が入っていないこと（科目の既定値 P10 を入れない）。
    const sent = [];
    page.on('request', (r) => r.method() === 'PATCH' && r.url().includes('/api/v1/drafts/') && sent.push(r.postDataJSON()));
    const rows = rowsOf(page);
    await accountOf(rows.nth(0)).click();
    await page.keyboard.insertText('つうしん');
    await page.keyboard.press('Enter');
    await expect(rows.nth(0)).toHaveClass(/is-candidate/);
    await expect(accountOf(rows.nth(1))).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(rows.nth(1)).toHaveClass(/is-candidate/);
    await expect.poll(async () => {
      const items = (await apiCall(page, 'GET', `/drafts?q=${encodeURIComponent(token)}`)).json.items;
      return items.map((x) => [x.account, x.tax_code]);
    }).toEqual([['exp_comm', null], ['exp_comm', null]]);
    expect(sent.filter((b) => b.tax_code)).toEqual([]);
  });
});
