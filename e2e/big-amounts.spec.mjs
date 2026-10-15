// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// AC-11.6 大きな金額（10桁以上・負の符号つき）でも、狭い幅のホームの数字のカードと、口座の合計・残高が、
// 枠の外にはみ出さず、隣の数字と重ならない（幅390の2列のカードで、10桁の金額がはみ出し、口座の合計では2つが重なった）。
// 帳簿には書き込まない（応えの金額を画面の中で大きくする）。未確認の下のバーは layout.spec.mjs の SC-03 が確かめる。

import { test, expect } from '@playwright/test';
import { open } from './helpers.mjs';

const BIG = 2467.31;
/** 応えの中の金額（1,000円以上）を大きくし、利益は負にする（負の符号の付いた長い数字を作る）。 */
function enlarge(v, key = '') {
  if (Array.isArray(v)) return v.map((x) => enlarge(x, key));
  if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, enlarge(x, k)]));
  if (typeof v !== 'number' || Math.abs(v) < 1000 || /count|draft|events|year/.test(key)) return v;
  const n = Math.round(v * BIG);
  return /profit|balance|total|estimate/.test(key) ? -Math.abs(n) : n;
}

/** 枠（.card）の外にはみ出した金額と、横に重なった金額。ページの横のスクロールも。 */
const problems = (page) =>
  page.evaluate(() => {
    const items = [...document.querySelectorAll('main .card .money')].map((el) => ({ el, r: el.getBoundingClientRect() })).filter((x) => x.r.width > 0);
    const outside = items
      .filter(({ el, r }) => {
        const f = el.closest('.card').getBoundingClientRect();
        return r.right > f.right + 0.5 || r.left < f.left - 0.5;
      })
      .map(({ el }) => el.textContent);
    const overlap = [];
    items.forEach((a, i) =>
      items.slice(i + 1).forEach((b) => {
        if (a.r.left < b.r.right - 0.5 && b.r.left < a.r.right - 0.5 && a.r.top < b.r.bottom - 0.5 && b.r.top < a.r.bottom - 0.5) overlap.push(`${a.el.textContent} / ${b.el.textContent}`);
      }),
    );
    return { outside, overlap, hscroll: document.documentElement.scrollWidth > document.documentElement.clientWidth };
  });

for (const width of [390, 768]) {
  test(`AC-11.6 幅${width}: 大きな金額でも、ホームの数字と口座の合計・残高が枠からはみ出さず、重ならない`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    for (const path of ['**/api/v1/reports/dashboard*', '**/api/v1/wallets']) {
      await page.route(path, async (route) => {
        const res = await route.fetch();
        await route.fulfill({ response: res, json: enlarge(await res.json()) });
      });
    }
    await open(page, '#/?month=2026-09');
    await expect(page.locator('.kpi-value').first()).toContainText(/\d{1,3}(,\d{3}){3,}/);
    expect(await problems(page)).toEqual({ outside: [], overlap: [], hscroll: false });
    await open(page, '#/wallets');
    await expect(page.locator('.wallet-totals dd').first()).toContainText(/-\d{1,3}(,\d{3}){3,}/);
    expect(await problems(page)).toEqual({ outside: [], overlap: [], hscroll: false });
  });
}

// 設計担当の連絡（2026-10-15）と同じ金額: 売上 1,234,567,890円、経費 1,500,000,000円と 987,654,321円（9月）→ 2026年の利益 -1,249,321,553円。
// 幅390の2列のカードで、この数字の右端が 403px になり、ページが 403>390 に広がった件（大きな金額では、枠の幅で数字の大きさを下げる）。
test('AC-11.1 AC-11.6 幅390: 2026年の利益 -1,249,321,553円 が、数字のカードの枠に収まり、ページが広がらない', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.route('**/api/v1/reports/dashboard*', async (route) => {
    const res = await route.fetch();
    const d = await res.json();
    const add = { revenue: 1234567890, expense: 1500000000 + 987654321 };
    const sum = (s) => ({ ...s, revenue: s.revenue + add.revenue, expense: s.expense + add.expense, profit: s.profit + add.revenue - add.expense });
    await route.fulfill({ response: res, json: { ...d, month_summary: { ...sum(d.month_summary), prev: d.month_summary.prev }, ytd: sum(d.ytd) } });
  });
  await open(page, '#/?month=2026-09');
  const card = page.locator('.kpi', { hasText: '年の利益' });
  await expect(card.locator('.kpi-value')).toContainText(/-1,\d{3},\d{3},\d{3}/);
  const { right, frame, scroll, client } = await card.evaluate((c) => ({
    right: c.querySelector('.kpi-value .money').getBoundingClientRect().right,
    frame: c.getBoundingClientRect().right,
    scroll: document.documentElement.scrollWidth,
    client: document.documentElement.clientWidth,
  }));
  expect(right).toBeLessThanOrEqual(frame);
  expect(scroll).toBeLessThanOrEqual(client);
  expect(await problems(page)).toEqual({ outside: [], overlap: [], hscroll: false });
});
