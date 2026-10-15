// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// AC-05.6 の材料: 100行を、キーボードだけで確定候補にして確定する。1行あたりのキー押下の数と、全体の自動実行の時間を出す。
// 人の所要時間の代わりにはならない（変換の操作、目で確かめる時間、迷う時間が入っていない）。人の計測は manual の項目。
// 数え方: press は1回、insertText は文字の数（かな入力で1文字1打鍵とみなす。ローマ字入力なら、おおよそ1.5〜2倍になる）。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, walletOf, importRows } from './helpers.mjs';

const ROWS = 100;
// よく使う科目を順に選ぶ（読みの先頭の数文字で、候補の先頭に来るもの）。
const PICKS = [
  ['しょうも', '消耗品費'],
  ['つうし', '通信費'],
  ['りょひ', '旅費交通費'],
  ['かいぎ', '会議費'],
];

test('AC-05.6 の材料: 100行をキーボードだけで確定候補にして確定する（キー押下の数と時間を表示）', async ({ page }, info) => {
  test.setTimeout(180_000);
  await open(page, '#/');
  const token = uniqueText('E2E 100行').replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  // 1月の日付にする（ほかの試験が10月に作る仕訳を、仕訳帳の1ページ目から押し出さないように）。
  const rows = Array.from({ length: ROWS }, (_, i) => [`2026-01-${String((i % 28) + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(500 + i * 10), '']);
  await importRows(page, card.code, rows);
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(page.locator('tr.inbox-row')).toHaveCount(ROWS);

  let keys = 0;
  const press = async (k) => {
    keys += 1;
    await page.keyboard.press(k);
  };
  const type = async (text) => {
    keys += [...text].length;
    await page.keyboard.insertText(text);
  };
  const started = Date.now();
  await press('/');
  // 検索欄 → 見出しの「すべて選ぶ」→ 候補の欄 → 日付の欄 → 科目の欄。
  for (let i = 0; i < 4; i += 1) await press('Tab');
  for (let i = 0; i < ROWS; i += 1) {
    const [reading, name] = PICKS[i % PICKS.length];
    await type(reading);
    await expect(page.getByRole('option', { name: new RegExp(name) }).first()).toHaveAttribute('aria-selected', 'true');
    await press('Enter');
  }
  await expect(page.locator('.inbox-bar')).toContainText(`確定する行${ROWS}件`);
  await press('ControlOrMeta+Enter');
  await expect(page.getByRole('dialog', { name: `${ROWS}件を確定しますか` })).toBeVisible();
  await press('Enter');
  await expect(page.locator('.toast').filter({ hasText: `${ROWS}件を確定しました` })).toBeVisible({ timeout: 20_000 });
  const seconds = (Date.now() - started) / 1000;

  const confirmed = (await apiCall(page, 'GET', `/drafts?status=confirmed&q=${encodeURIComponent(token)}&limit=500`)).json.total;
  expect(confirmed).toBe(ROWS);
  const perRow = (keys / ROWS).toFixed(2);
  const text = `100行: キー押下 ${keys}回（1行あたり ${perRow}回）、自動実行の時間 ${seconds.toFixed(1)}秒。人の所要時間ではありません。`;
  info.annotations.push({ type: 'AC-05.6 の材料', description: text });
  console.log(text);
  // 1行あたり、科目の読み（3〜4文字）と Enter の1回で進めること（確定候補にする操作を別に要さない）。
  expect(keys / ROWS).toBeLessThan(6);
});
