// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 設定（SC-10）: 台帳の検査（AC-09.1）、末尾のハッシュの写し取りと、控えたハッシュの照合（AC-09.2）、事業者の保存、
// 期首残高の見直し（仕訳帳の詳細からの案内、ほかの場所の書き込みで断られたときに入力を保つ）、見た目。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, addCashExpense } from './helpers.mjs';

test('AC-09.1 台帳を検査すると結果が出る。壊れていれば最初の連番と規則を示す', async ({ page }) => {
  await open(page, '#/settings?section=data');
  await page.getByRole('button', { name: '台帳を検査する' }).click();
  const out = page.locator('.settings-data .check-out').first();
  await expect(out).toContainText('台帳は壊れていません');
  await expect(out).toContainText(/\d+件の記録を最初から順にたどり/);
  await page.route('**/api/v1/verify', (route) => route.fulfill({ json: { ok: false, error: { seq: 55, kind: 'bad_hash', rule: null } } }));
  await page.getByRole('button', { name: '台帳を検査する' }).click();
  await expect(out).toContainText('台帳に壊れた記録があります');
  await expect(out).toContainText('55番目の記録に、ハッシュの不一致');
});

test('AC-09.2 末尾のハッシュを写し取れて、控えたハッシュを入れると台帳の何番目にあたるかが分かる', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, '#/settings?section=data');
  const status = (await apiCall(page, 'GET', '/status')).json;
  await expect(page.locator('.hash-full')).toHaveText(status.ledger.head_hash);
  await page.getByRole('button', { name: '写し取る' }).click();
  await expect(page.locator('.toast').filter({ hasText: '写し取りました' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(status.ledger.head_hash);

  const input = page.getByLabel('控えたハッシュ');
  const out = page.locator('.settings-data .check-out').last();
  // 末尾のハッシュ（大文字と前後の空白を混ぜても読める）。
  await input.fill(` ${status.ledger.head_hash.toUpperCase()} `);
  await page.keyboard.press('Enter');
  await expect(out).toContainText(`台帳の${status.ledger.head_seq}番目の記録と一致しました（末尾と同じです）`);
  // 前に控えた、途中の記録のハッシュ。
  const list = (await apiCall(page, 'GET', '/entries?state=all&limit=500')).json.items;
  const oldest = list[list.length - 1];
  const ev = (await apiCall(page, 'GET', `/entries/${oldest.id}`)).json.history[0];
  await input.fill(ev.hash);
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(out).toContainText(`台帳の${ev.seq}番目の記録と一致しました（末尾ではありません）`);
  await expect(out).toContainText(`${status.ledger.events - ev.seq}件の記録が足されています`);
  // どの記録とも一致しない。
  await input.fill('0'.repeat(64));
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(out).toContainText('一致する記録がありません');
  // 形が違えば、欄の下に理由。
  await input.fill('abc');
  await page.getByRole('button', { name: '照合する' }).click();
  await expect(page.locator('#set-data .field-error').filter({ hasText: '64文字です（いまは3文字）' })).toBeVisible();
});

test('SC-10 事業者の設定を保存できる。免税にすると登録番号の欄を隠す', async ({ page }) => {
  await open(page, '#/settings');
  const before = (await apiCall(page, 'GET', '/settings')).json;
  try {
    const name = page.getByLabel('事業者名');
    await name.fill('デモ事務所（試験）');
    await page.getByRole('button', { name: '事業者の設定を保存' }).click();
    await expect(page.locator('.toast').filter({ hasText: '設定を保存しました' })).toBeVisible();
    await expect(page.locator('.sidebar-business')).toHaveText('デモ事務所（試験）');
    expect((await apiCall(page, 'GET', '/settings')).json.business_name).toBe('デモ事務所（試験）');
    await page.locator('#set-business .seg-option', { hasText: '課税事業者' }).click();
    await expect(page.getByLabel('登録番号（適格請求書発行事業者）')).toBeVisible();
    await page.getByLabel('登録番号（適格請求書発行事業者）').fill('T123');
    await page.getByRole('button', { name: '事業者の設定を保存' }).click();
    await expect(page.locator('#set-business .field-error').filter({ hasText: 'T と13桁' })).toBeVisible();
    await page.locator('#set-business .seg-option', { hasText: '免税事業者' }).click();
    await expect(page.getByLabel('登録番号（適格請求書発行事業者）')).toBeHidden();
  } finally {
    await apiCall(page, 'PUT', '/settings', before);
  }
});

test('期首残高の仕訳は、仕訳帳の詳細で訂正・取消を出さず、設定の「期首残高」へ案内する。設定から見直せる', async ({ page }) => {
  await open(page, '#/');
  const opening = (await apiCall(page, 'GET', '/opening')).json;
  test.skip(!opening.entry_id, '期首残高の仕訳が無い帳簿');
  await open(page, `#/journal/${opening.entry_id}`);
  const drawer = page.getByRole('dialog');
  await expect(drawer).toContainText('期首残高は、設定の「期首残高」から見直します');
  await expect(drawer.getByRole('button', { name: /訂正する/ })).toHaveCount(0);
  await expect(drawer.getByRole('button', { name: /取り消す/ })).toHaveCount(0);
  // 仕訳帳の e と v も働かない（詳細の中でも）。
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/journal/${opening.entry_id}`));
  await drawer.getByRole('link', { name: '設定の「期首残高」を開く' }).click();
  await expect(page).toHaveURL(/#\/settings\?section=opening/);
  await expect(page.locator('#set-opening h2')).toBeFocused();
  const first = opening.balances[0];
  const field = page.locator('#set-opening input[aria-label$="の残高"]').first();
  const original = await field.inputValue();
  try {
    await field.fill(String(first.amount + 1000));
    await page.getByRole('button', { name: '期首残高を保存' }).click();
    await expect(page.locator('.toast').filter({ hasText: '期首残高を保存しました' })).toBeVisible();
    const after = (await apiCall(page, 'GET', '/opening')).json;
    expect(after.entry_id).toBe(opening.entry_id);
    expect(after.capital).toBe(opening.capital + 1000);
    const detail = (await apiCall(page, 'GET', `/entries/${opening.entry_id}`)).json;
    expect(detail.history.at(-1).reason).toBe('期首残高の見直し');
  } finally {
    await apiCall(page, 'PUT', '/opening', { date: opening.date, balances: opening.balances });
    expect(original).not.toBe('');
  }
});

test('409 stale: 期首残高の保存が、ほかの場所の書き込みで断られたら「他の場所で更新されました」と出し、入力を失わない', async ({ page }) => {
  await open(page, '#/settings?section=opening');
  const before = (await apiCall(page, 'GET', '/opening')).json;
  // 保存（PUT）だけを、別のプロセスが先に書いたときの応えにする。読み込み（GET）はそのまま通す。
  await page.route('**/api/v1/opening', (route) =>
    route.request().method() === 'PUT'
      ? route.fulfill({ status: 409, json: { error: { code: 'stale', message: '台帳がほかの場所で更新されました。', field: null } } })
      : route.continue(),
  );
  const field = page.locator('#set-opening input[aria-label$="の残高"]').first();
  await field.fill('123456');
  await page.getByRole('button', { name: '期首残高を保存' }).click();
  await expect(page.locator('.toast').filter({ hasText: '他の場所で更新されました' })).toBeVisible();
  await expect(field).toHaveValue(/123,?456/);
  expect((await apiCall(page, 'GET', '/opening')).json).toEqual(before);
});

test('AC-08.6 の画面側: Shift_JIS で書き出すと、無い文字を「?」に置き換えたことを知らせ、ファイルは保存される', async ({ page }) => {
  await open(page, '#/settings');
  // Shift_JIS に無い文字（𠮷）を摘要に入れた仕訳を作る。
  await addCashExpense(page, { description: uniqueText('E2E 𠮷の書き出し') });
  await page.getByLabel('仕訳帳のCSVの文字コード').selectOption('sjis');
  const link = page.getByRole('link', { name: '仕訳帳のCSV' });
  await expect(link).toHaveAttribute('href', /encoding=sjis/);
  const download = page.waitForEvent('download');
  await link.click();
  expect((await download).suggestedFilename()).toMatch(/\.csv$/);
  const warned = page.locator('.toast').filter({ hasText: '「?」に置き換えて書き出しました' });
  await expect(warned).toBeVisible();
  // UTF-8 なら置き換えは起きないので、知らせは増えない（前の知らせは、時間で消えることがある）。
  await page.getByLabel('仕訳帳のCSVの文字コード').selectOption('utf8bom');
  const before = await warned.count();
  const second = page.waitForEvent('download');
  await link.click();
  expect((await second).suggestedFilename()).toMatch(/\.csv$/);
  await page.waitForTimeout(300);
  expect(await warned.count()).toBeLessThanOrEqual(before);
});

test('SC-10 見た目: ライト・ダーク・OSに合わせるを選べて、ブラウザの中に残る', async ({ page }) => {
  await open(page, '#/settings?section=appearance');
  await page.locator('#set-appearance .seg-option', { hasText: 'ダーク' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.locator('#set-appearance .seg-option', { hasText: '狭い' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await page.locator('#set-appearance .seg-option', { hasText: 'OSに合わせる' }).click();
  await page.locator('#set-appearance .seg-option', { hasText: '標準' }).click();
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /./);
  await expect(page.locator('html')).not.toHaveAttribute('data-density', /./);
});
