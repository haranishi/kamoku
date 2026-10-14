// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 登録簿（SC-07 口座、SC-08 勘定科目、SC-09 取引先）: 一覧、追加・編集のダイアログ、型の選択、名前の検索、
// 無効の切り替え、名前を変えられない科目、登録番号の形式の誤りを欄の下に出す。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText } from './helpers.mjs';

test('SC-07 口座: 残高（カードは未払い額）と、元帳・未確認へのリンク。型を選んで足し、使わない口座にできる', async ({ page }) => {
  await open(page, '#/wallets');
  const list = page.locator('.wallet-list').first();
  const card = list.locator('.wallet-item', { has: page.locator('.wallet-balance', { hasText: '未払い額' }) }).first();
  await expect(card).toBeVisible();
  const href = await card.getByRole('link', { name: /未確認/ }).getAttribute('href');
  expect(href).toMatch(/^#\/inbox\?wallet=/);
  await expect(card.getByRole('link', { name: 'この口座の元帳' })).toHaveAttribute('href', /^#\/reports\/ledger\?account=/);

  const name = uniqueText('デモ電子マネー');
  await page.getByRole('button', { name: '口座を足す' }).click();
  const dialog = page.getByRole('dialog', { name: '口座を足す' });
  await dialog.locator('.seg-option', { hasText: '電子マネー' }).click();
  await dialog.getByRole('textbox', { name: /^名前/ }).fill(name);
  await page.keyboard.press('Enter');
  await expect(page.locator('.toast').filter({ hasText: `口座「${name}」を足しました` })).toBeVisible();
  const item = page.locator('.wallet-item', { hasText: name });
  await expect(item).toContainText('電子マネー');
  const saved = (await apiCall(page, 'GET', '/accounts')).json.items.find((a) => a.name === name);
  expect([saved.wallet.type, saved.kind, saved.parent]).toEqual(['ewallet', 'asset', 'cash']);

  await item.getByRole('button', { name: `「${name}」を直す` }).click();
  const edit = page.getByRole('dialog', { name: `「${name}」を直す` });
  await edit.getByRole('checkbox', { name: 'この口座を使う' }).uncheck();
  await edit.getByRole('button', { name: '保存する' }).click();
  await expect(page.locator('.masters-off')).toContainText(name);
});

test('SC-08 勘定科目: 分類ごとに並び、読みで探せる。既定の割合を直せる。名前を変えられない科目と、外せない科目', async ({ page }) => {
  await open(page, '#/accounts');
  for (const k of ['資産', '負債', '資本', '収益', '費用']) await expect(page.getByRole('heading', { name: new RegExp(`^${k}`) })).toBeVisible();
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('つうしん');
  const rows = page.locator('.accounts-table tbody tr');
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('通信費');
  // 行で Enter を押すと、直すダイアログ。
  await rows.first().focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByRole('dialog', { name: '「通信費」を直す' });
  const pct = dialog.getByRole('textbox', { name: '既定の事業割合' });
  const before = await pct.inputValue();
  await pct.fill('60');
  await dialog.getByRole('button', { name: '保存する' }).click();
  await expect(page.locator('.toast').filter({ hasText: '「通信費」を保存しました' })).toBeVisible();
  expect((await apiCall(page, 'GET', '/accounts')).json.items.find((a) => a.code === 'exp_comm').default_pct).toBe(60);
  await apiCall(page, 'PATCH', '/accounts/exp_comm', { default_pct: Number(before) });

  // 事業主貸は名前を変えられない（欄が無効）。売上高は外せない。
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('じぎょうぬしかし');
  await page.locator('.accounts-table tbody tr').first().click();
  const owner = page.getByRole('dialog', { name: '「事業主貸」を直す' });
  await expect(owner.getByRole('textbox', { name: /^名前/ })).toBeDisabled();
  await expect(owner).toContainText('名前を変えられません');
  await owner.getByRole('button', { name: 'やめる' }).click();
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('うりあげ');
  await page.locator('.accounts-table tbody tr').first().click();
  const sales = page.getByRole('dialog', { name: '「売上高」を直す' });
  await expect(sales.getByRole('checkbox', { name: 'この科目を使う' })).toBeDisabled();
  await sales.getByRole('button', { name: 'やめる' }).click();

  // 使っていない科目は、切り替えで出る。
  await page.getByRole('searchbox', { name: '科目を探す' }).fill('とうざ');
  await expect(page.locator('.masters')).toContainText('当てはまる科目はありません');
  await page.getByRole('checkbox', { name: '使っていない科目も出す' }).check();
  await expect(page.locator('.accounts-table tbody tr').first()).toContainText('当座預金');
  await expect(page.locator('.accounts-table tbody tr').first()).toContainText('使っていない');
});

test('SC-09 取引先: 登録番号の形式の誤りを欄の下に出し、全角や小文字でもそろえて保存する。同じ名前は断る', async ({ page }) => {
  await open(page, '#/partners');
  const name = uniqueText('デモ取引先');
  await page.getByRole('button', { name: '取引先を足す' }).click();
  const dialog = page.getByRole('dialog', { name: '取引先を足す' });
  await dialog.getByRole('textbox', { name: /^名前/ }).fill(name);
  const inv = dialog.getByRole('textbox', { name: /登録番号/ });
  await inv.fill('T123');
  await dialog.getByRole('button', { name: '足す' }).click();
  const err = dialog.locator('.field-error').filter({ hasText: 'T と13桁' });
  await expect(err).toBeVisible();
  await expect(inv).toHaveAttribute('aria-invalid', 'true');
  await expect(inv).toBeFocused();
  await inv.fill('ｔ１２３４５６７８９０１２３');
  await dialog.getByRole('button', { name: '足す' }).click();
  await expect(page.locator('.toast').filter({ hasText: `取引先「${name}」を足しました` })).toBeVisible();
  const saved = (await apiCall(page, 'GET', '/partners')).json.items.find((p) => p.name === name);
  expect(saved.invoice_number).toBe('T1234567890123');
  await expect(page.locator('.partners-table')).toContainText(name);

  // 同じ名前は、サーバーが断る。理由は名前の欄の下に出る。
  await page.getByRole('button', { name: '取引先を足す' }).click();
  const again = page.getByRole('dialog', { name: '取引先を足す' });
  await again.getByRole('textbox', { name: /^名前/ }).fill(name);
  await again.getByRole('button', { name: '足す' }).click();
  await expect(again.locator('.field-error').filter({ hasText: 'すでにあります' })).toBeVisible();
  await again.getByRole('button', { name: 'やめる' }).click();
});
