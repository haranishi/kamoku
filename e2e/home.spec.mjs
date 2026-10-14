// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ホーム（SC-01）: 次にやることが一番上、赤字の文字、台帳のハッシュの写し取り、未確認が無いときの次の候補。

import { test, expect } from '@playwright/test';
import { open, apiCall } from './helpers.mjs';

test('AC-07.1 最初に見えるのは「次にやること」で、数字の一覧より上にある', async ({ page }) => {
  await open(page, '#/');
  const next = page.locator('.next-card');
  await expect(next).toContainText('未確認の取引が');
  await expect(next.getByRole('link', { name: /未確認を開く/ })).toHaveAttribute('href', '#/inbox');
  const nextBox = await next.boundingBox();
  const kpiBox = await page.locator('.kpis').boundingBox();
  expect(nextBox.y + nextBox.height).toBeLessThanOrEqual(kpiBox.y);
});

test('AC-07.4 利益が負の月は、色だけでなく「赤字」の文字と符号で示す', async ({ page }) => {
  await open(page, '#/');
  const dash = (await apiCall(page, 'GET', '/reports/dashboard')).json;
  const loss = dash.series.find((m) => m.profit < 0);
  test.skip(!loss, '赤字の月が無いデータ');
  await page.goto(`/#/?month=${loss.month}`);
  const card = page.locator('.kpi', { hasText: `${Number(loss.month.slice(5))}月の利益` });
  await expect(card).toContainText('赤字');
  await expect(card.locator('.kpi-value')).toContainText('-');
  await expect(page.locator('.month-select')).toHaveValue(loss.month);
  // 戻る操作で、前の月（今月）に戻る。
  await page.goBack();
  await expect(page.locator('.month-select')).toHaveValue(dash.month);
});

test('AC-09.2 の入口: 台帳の末尾のハッシュを写し取れる。写せない環境では全文を選んで手で写せる', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await open(page, '#/');
  const status = (await apiCall(page, 'GET', '/status')).json;
  const card = page.locator('section', { has: page.getByRole('heading', { name: '台帳の状態' }) });
  await expect(card).toContainText(status.ledger.head_hash.slice(0, 8));
  await card.getByRole('button', { name: '写し取る' }).click();
  await expect(page.locator('.toast').filter({ hasText: '写し取りました' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(status.ledger.head_hash);

  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('denied'));
  });
  await card.getByRole('button', { name: '写し取る' }).click();
  const field = card.getByRole('textbox', { name: '末尾のハッシュ（全文）' });
  await expect(field).toBeFocused();
  await expect(field).toHaveValue(status.ledger.head_hash);
  expect(await field.evaluate((el) => [el.selectionStart, el.selectionEnd])).toEqual([0, 64]);
});

test('未確認が0件なら「未確認はありません」と、次の候補を1つ出す', async ({ page }) => {
  await page.route('**/api/v1/reports/dashboard*', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.next_actions = { inbox_draft: 0, inbox_out_amount: 0 };
    await route.fulfill({ response: res, json: body });
  });
  await open(page, '#/');
  const next = page.locator('.next-card');
  await expect(next).toContainText('未確認はありません。');
  await expect(next.getByRole('link').first()).toHaveAttribute('href', /#\/(entry|reports)/);
});
