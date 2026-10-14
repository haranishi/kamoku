// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳帳（SC-05）: キーで行を動かして詳細を開く、訂正（理由と差分）、取消（理由と赤系の確認）、古い版からの訂正。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, ensureEntryWith } from './helpers.mjs';

async function createEntry(page, desc, amount = 3300) {
  const res = await apiCall(page, 'POST', '/entries', {
    kind: 'expense',
    date: '2026-10-02',
    amount,
    account: 'exp_supplies',
    wallet: 'w01',
    tax_code: 'P10',
    partner: 'サンプル文具',
    description: desc,
    business_pct: 100,
    memo: '',
  });
  expect(res.status).toBe(201);
  return res.json;
}

test('SC-05 j k で行を動かし、Enterで詳細を開き、Escで閉じると一覧と同じ行に戻る', async ({ page }) => {
  await open(page, '#/journal');
  const rows = page.locator('.journal-table tbody tr');
  await expect(rows.first()).toBeVisible();
  await page.keyboard.press('j');
  await expect(rows.nth(0)).toBeFocused();
  await page.keyboard.press('j');
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press('k');
  await page.keyboard.press('j');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal\/e_[0-9a-f]+/);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('の仕訳');
  await expect(drawer.getByRole('heading', { name: '履歴' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(drawer).toHaveCount(0);
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(rows.nth(1)).toBeFocused();
});

test('SC-05 絞り込みはアドレスに入り、戻る操作で前の条件に戻る', async ({ page }) => {
  await open(page, '#/journal');
  // 本物のサーバーの合成データには「家賃」の仕訳が無いことがあるので、無ければ作る。
  await ensureEntryWith(page, '家賃');
  await page.reload();
  await page.locator('main h1').focus();
  await page.keyboard.press('/');
  const search = page.getByRole('searchbox', { name: '文字で探す' });
  await expect(search).toBeFocused();
  await search.fill('家賃');
  await search.press('Enter');
  await expect(page).toHaveURL(/#\/journal\?q=%E5%AE%B6%E8%B3%83/);
  await expect(page.locator('.journal-table tbody tr').first()).toContainText('家賃');
  await page.locator('main h1').focus();
  await page.keyboard.press(']');
  await expect(page).toHaveURL(/from=\d{4}-\d{2}-01/);
  await page.goBack();
  await expect(page).toHaveURL(/#\/journal\?q=%E5%AE%B6%E8%B3%83$/);
  await expect(search).toHaveValue('家賃');
});

test('AC-06.4 詳細から訂正に移ると現在の値が入っていて、理由と差分を確かめて第2版になる', async ({ page }) => {
  const desc = uniqueText('E2E 訂正');
  const e = await createEntry(page, desc);
  await open(page, `#/journal/${e.id}`);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText(desc);
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/entry/${e.id}$`));
  const f = entryFields(page);
  await expect(page.locator('main h1')).toHaveText('取引を訂正');
  await expect(f.amount).toHaveValue('3,300');
  await expect(f.account).toHaveValue('消耗品費');
  await expect(f.wallet).toHaveValue('現金');
  await expect(f.description).toHaveValue(desc);
  await expect(f.date).toHaveValue('2026-10-02 (金)');

  // 理由が無いと保存できない。
  await f.amount.fill('3850');
  await page.keyboard.press('ControlOrMeta+Enter');
  await expect(page.getByText('訂正の理由を入れてください。')).toBeVisible();
  await page.getByLabel('訂正の理由').fill('金額の入力ミス');
  await page.keyboard.press('ControlOrMeta+Enter');
  const confirm = page.getByRole('dialog', { name: /を訂正します/ });
  await expect(confirm).toContainText('3,300円');
  await expect(confirm).toContainText('3,850円');
  await expect(confirm.getByRole('button', { name: '訂正を保存' })).toBeFocused();
  await page.keyboard.press('Enter');

  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}$`));
  await expect(page.getByRole('dialog')).toContainText('訂正 第2版');
  await expect(page.getByRole('dialog')).toContainText('理由: 金額の入力ミス');
  const after = (await apiCall(page, 'GET', `/entries/${e.id}`)).json;
  expect(after.revision).toBe(2);
  expect(after.total).toBe(3850);
});

test('AC-06.6 ほかの場所で更新されていたら、読み直して差分を見せ、入力は残す', async ({ page }) => {
  const desc = uniqueText('E2E 競合');
  const e = await createEntry(page, desc);
  await open(page, `#/entry/${e.id}`);
  const f = entryFields(page);
  await expect(f.description).toHaveValue(desc);
  await f.description.fill(`${desc} 画面で直した`);
  // 画面が読み込んだ後に、別の場所（ここではAPI）で第2版にする。
  const other = await apiCall(page, 'PUT', `/entries/${e.id}`, { ...e.input, amount: 4400, reason: '別の場所での訂正', expected_revision: 1 });
  expect(other.status).toBe(200);

  await page.getByLabel('訂正の理由').fill('摘要を直した');
  await page.getByRole('button', { name: /訂正を確かめる/ }).click();
  await page.getByRole('button', { name: '訂正を保存' }).click();
  await expect(page.locator('.toast').filter({ hasText: '他の場所で更新されました。読み込み直します。' })).toBeVisible();
  const box = page.locator('.stale-box');
  await expect(box).toContainText('最新の版（第2版）を読み込みました');
  await expect(box).toContainText('4,400円');
  await expect(f.description).toHaveValue(`${desc} 画面で直した`);

  // もう一度保存すると、最新の版を元に第3版になる。
  await page.getByRole('button', { name: /訂正を確かめる/ }).click();
  await page.getByRole('button', { name: '訂正を保存' }).click();
  await expect(page).toHaveURL(new RegExp(`#/journal/${e.id}$`));
  const after = (await apiCall(page, 'GET', `/entries/${e.id}`)).json;
  expect(after.revision).toBe(3);
  expect(after.description).toBe(`${desc} 画面で直した`);
});

test('SC-05 取消は理由が要り、赤系の確認ボタンで取り消す', async ({ page }) => {
  const desc = uniqueText('E2E 取消');
  const e = await createEntry(page, desc);
  await open(page, `#/journal/${e.id}`);
  await expect(page.getByRole('dialog')).toContainText(desc);
  await page.keyboard.press('v');
  const dialog = page.getByRole('dialog', { name: '仕訳を取り消しますか' });
  await expect(dialog).toBeVisible();
  const confirm = dialog.getByRole('button', { name: '取り消す' });
  await expect(confirm).toHaveClass(/btn--danger/);
  await confirm.click();
  await expect(dialog).toContainText('取消の理由を入れてください。');
  await dialog.getByLabel('取消の理由').fill('二重に登録したため');
  await confirm.click();
  await expect(dialog).toHaveCount(0);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('取消済み');
  await expect(drawer.getByRole('button', { name: /訂正する/ })).toBeDisabled();
  await expect(drawer.getByRole('button', { name: /取り消す/ })).toBeDisabled();
  expect((await apiCall(page, 'GET', `/entries/${e.id}`)).json.state).toBe('voided');
  await page.keyboard.press('Escape');
  await expect(page.locator('.journal-table tbody tr', { hasText: desc })).toContainText('取消済み');
});
