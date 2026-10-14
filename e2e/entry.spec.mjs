// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録（SC-02）。キーボードだけの登録、続けて入力、按分とプレビュー、金額の式、日付の形。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, withSettings, withAccountDefaults } from './helpers.mjs';

test('AC-02.1 AC-02.5 キーボードだけで1件登録し、種別・日付・口座を残して金額へ戻る', async ({ page }) => {
  const desc = uniqueText('E2E 携帯電話');
  await open(page, '#/entry');
  // 通信費の既定値（課税仕入10%・割合70%）を、この試験の間だけ入れる（本物の合成データは割合100%のため）。
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 70 }, async () => {
    // 画面は科目を開いたときに読むので、読み込み直して新しい既定値を使わせる。
    await page.reload();
    await expect(page.locator('main h1')).toBeVisible();
    const f = entryFields(page);
    await expect(page.locator('main h1')).toBeFocused();

    // 種別の欄へ（1 で支出）。ここからはEnterだけで欄を進める。
    await page.keyboard.press('1');
    await expect(f.kind('支出')).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(f.date).toBeFocused();
    await page.keyboard.type('1005');
    await page.keyboard.press('Enter');
    await expect(f.amount).toBeFocused();
    await page.keyboard.type('11000');
    await page.keyboard.press('Enter');
    await expect(f.account).toBeFocused();
    await page.keyboard.insertText('つうしん');
    await expect(page.getByRole('option', { name: /通信費/ })).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(f.account).toHaveValue('通信費');
    await expect(f.wallet).toBeFocused();
    await page.keyboard.insertText('サンプルカード');
    await page.keyboard.press('Enter');
    await expect(f.wallet).toHaveValue('サンプルカード');
    await expect(f.tax).toBeFocused();
    await page.keyboard.press('Enter');
    await expect(f.partner).toBeFocused();
    await page.keyboard.insertText('サンプル通信');
    await page.keyboard.press('Enter');
    await expect(f.description).toBeFocused();
    await page.keyboard.insertText(desc);
    await page.keyboard.press('Enter');
    await expect(f.pct).toBeFocused();
    // 通信費の既定の割合（70%）が入っていて、プレビューに事業主貸 3,300円が出ている。
    await expect(f.pct).toHaveValue('70');
    await expect(f.preview).toContainText('3,300');
    await page.keyboard.press('Enter');

    await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
    // 続けて入力: 金額へ戻り、種別・日付・口座は残る（AC-02.5）。
    await expect(f.amount).toBeFocused();
    await expect(f.amount).toHaveValue('');
    await expect(f.kind('支出')).toBeChecked();
    await expect(f.date).toHaveValue(/-10-05/);
    await expect(f.wallet).toHaveValue('サンプルカード');
    await expect(f.account).toHaveValue('');

    // 保存された仕訳は、プレビューと同じ明細（AC-02.2 の画面側）。
    const res = await apiCall(page, 'GET', `/entries?q=${encodeURIComponent(desc)}&state=all`);
    expect(res.json.total).toBe(1);
    const lines = res.json.items[0].lines.map((l) => [l.account, l.side, l.amount, l.tax_amount]);
    expect(lines).toEqual([
      ['exp_comm', 'debit', 7700, 700],
      ['owner_draw', 'debit', 3300, 0],
      ['w03', 'credit', 11000, 0],
    ]);
  });
});

test('AC-03.4 割合を変えると、仕訳のプレビューも変わる', async ({ page }) => {
  await open(page, '#/entry');
  // 通信費の既定値（課税仕入10%・割合70%）を、この試験の間だけ入れる（本物の合成データは割合100%のため）。
  await withAccountDefaults(page, 'exp_comm', { default_tax: 'P10', default_pct: 70 }, async () => {
    // 画面は科目を開いたときに読むので、読み込み直して新しい既定値を使わせる。
    await page.reload();
    await expect(page.locator('main h1')).toBeVisible();
    const f = entryFields(page);
    await f.amount.fill('11000');
    await f.account.click();
    await page.keyboard.insertText('通信費');
    await page.keyboard.press('Enter');
    await expect(f.pct).toHaveValue('70');
    await expect(f.preview).toContainText('7,700');
    await expect(f.preview).toContainText('3,300');
    await f.pct.fill('50');
    await expect(f.preview).toContainText('5,500');
    await expect(f.preview).not.toContainText('7,700');
    await expect(page.locator('.pct-out')).toHaveText('→ 事業主貸 5,500円');
    await f.pct.fill('100');
    await expect(f.preview).not.toContainText('事業主貸');
  });
});

test('AC-02.3 金額に「1000+650」を入れると、離れたときに 1,650 になる', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('1000+650');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('1,650');
  await expect(f.form).toContainText('1000+650 = 1,650円');
  await f.amount.fill('１,６５０');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('1,650');
  await f.amount.fill('100/3');
  await f.amount.press('Tab');
  await expect(f.amount).toHaveValue('33');
  await expect(f.form).toContainText('切り捨てました');
});

test('AC-02.4 日付に「1005」を入れると、その年の10月5日になる。t と上下の矢印も効く', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.date.click();
  await f.date.fill('1005');
  await f.date.press('Tab');
  await expect(f.date).toHaveValue(/^\d{4}-10-05 \(.\)$/);
  await f.date.focus();
  await page.keyboard.press('ArrowUp');
  await expect(f.date).toHaveValue(/-10-06$/);
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('ArrowDown');
  await expect(f.date).toHaveValue(/-10-04$/);
  await page.keyboard.press('t');
  const today = await page.evaluate(() => {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  });
  await expect(f.date).toHaveValue(today);
  await f.date.fill('0230');
  await f.date.press('Tab');
  await expect(f.form).toContainText('2月30日はありません');
});
