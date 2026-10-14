// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 初期設定（SC-00、AC-01.1 AC-01.2 AC-01.4）。空の帳簿のサーバー（8768、--fresh）で、最後まで通る。
// 途中でリロードしても、決めた分が残る（各段階でサーバーに保存するため）。初期設定が済むまでは、どの画面に入っても初期設定へ移る。

import { test, expect } from '@playwright/test';
import { apiCall, uniqueText, openSetup, SETUP_URL, entryFields } from './helpers.mjs';

test.use({ baseURL: SETUP_URL });

test('AC-01.1 AC-01.2 AC-01.4 初期設定を最後まで通る。途中でリロードしても決めた分が残り、免税なら税区分の欄が隠れる', async ({ page }) => {
  await openSetup(page, '#/');
  // もう一度流すときのために、前の実行の分を戻す（済んでいたら「済んでいない」に。口座は残ったまま）。
  const s = (await apiCall(page, 'GET', '/settings')).json;
  if (s.setup_done) expect((await apiCall(page, 'PUT', '/settings', { ...s, setup_done: false })).status).toBe(200);
  // 前の実行で入れた期首残高を空にする（元入金の確かめを、この実行の数字だけにするため）。配置の試験（layout.spec）が
  // 先に「済んでいない」に戻すことがあるので、済んだかどうかに関係なく見る（本物のサーバーは帳簿が残り続ける）。
  const op = (await apiCall(page, 'GET', '/opening')).json;
  if (op.balances.length) {
    const cleared = await apiCall(page, 'PUT', '/opening', { date: op.date, balances: [] });
    expect(cleared.status, JSON.stringify(cleared.json)).toBe(200);
  }
  // アドレスの # だけを変えても読み込み直さないので、画面が覚えている状態を捨てる。
  await page.reload();
  // どの画面に入っても、初期設定へ移る。
  await openSetup(page, '#/journal');
  await expect(page).toHaveURL(/#\/setup/);
  await expect(page.locator('main h1')).toHaveText('初期設定');
  await expect(page.locator('.sidebar')).toBeHidden();

  // (1) 事業者名と課税の区分。名前が空なら進まない。
  const name = page.getByLabel('事業者名');
  await name.fill('');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page.locator('.setup-card .field-error').filter({ hasText: '事業者名を入れてください' })).toBeVisible();
  await name.fill('デモ商店（試験）');
  await page.locator('.seg-option', { hasText: '免税事業者' }).click();
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=2/);
  await expect(page.getByRole('heading', { name: '2. 口座' })).toBeVisible();

  // 途中でリロードしても、決めた分は残っている（AC-01.1）。
  await page.reload();
  await expect(page.getByRole('heading', { name: '2. 口座' })).toBeVisible();
  const saved = (await apiCall(page, 'GET', '/settings')).json;
  expect([saved.business_name, saved.tax_mode, saved.setup_done]).toEqual(['デモ商店（試験）', 'exempt', false]);
  await expect(page.locator('.setup-wallets')).toContainText('現金');

  // (2) 口座は型を選んで足し、名前は自由に付けられる（AC-01.2）。
  const bank = uniqueText('デモ銀行');
  const card = uniqueText('デモカード');
  for (const [type, label] of [['預金', bank], ['クレジットカード', card]]) {
    await page.getByRole('button', { name: '口座を足す' }).click();
    const dialog = page.getByRole('dialog', { name: '口座を足す' });
    await dialog.locator('.seg-option', { hasText: type }).click();
    await dialog.getByRole('textbox', { name: /^名前/ }).fill(label);
    await dialog.getByRole('button', { name: '足す' }).click();
    await expect(dialog).toHaveCount(0);
    await expect(page.locator('.setup-wallets')).toContainText(label);
  }
  const wallets = (await apiCall(page, 'GET', '/accounts')).json.items.filter((a) => a.wallet);
  expect(wallets.find((w) => w.name === bank).wallet.type).toBe('bank');
  expect(wallets.find((w) => w.name === card).wallet.type).toBe('card');
  await page.getByRole('button', { name: '次へ' }).click();

  // (3) 期首残高。カードは未払い額（負債）。元入金は差額。
  await expect(page.getByRole('heading', { name: '3. 期首残高' })).toBeVisible();
  await page.getByLabel(`${bank}の残高`).fill('500000');
  await page.getByLabel(`${card}の残高`).fill('12000');
  await page.getByLabel('現金の残高').fill('30000');
  await page.getByLabel('現金の残高').press('Tab');
  await expect(page.locator('.opening-capital')).toContainText('518,000');
  await page.getByRole('button', { name: '保存して次へ' }).click();
  await expect(page).toHaveURL(/step=4/);

  // (4) 確認して始める。リロードしても、保存した内容が並ぶ。
  await page.reload();
  const summary = page.locator('.setup-summary');
  await expect(summary).toContainText('デモ商店（試験）');
  await expect(summary).toContainText('免税事業者');
  await expect(summary).toContainText(bank);
  await expect(summary).toContainText('元入金 518,000円');
  await page.getByRole('button', { name: '記帳を始める' }).click();
  await expect(page).toHaveURL(/#\/$/);
  await expect(page.locator('.next-card')).toBeVisible();
  await expect(page.locator('.sidebar')).toBeVisible();
  expect((await apiCall(page, 'GET', '/status')).json.setup_done).toBe(true);

  // 免税なので、取引の登録で税区分の欄が隠れている（AC-01.4）。
  await page.goto(`${SETUP_URL}/#/entry`);
  const f = entryFields(page);
  await expect(f.amount).toBeVisible();
  await expect(f.tax).toBeHidden();
  await expect(page.getByRole('button', { name: '詳細: 税区分を出す' })).toBeVisible();
});
