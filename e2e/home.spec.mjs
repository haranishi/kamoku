// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ホーム（SC-01）: 次にやることが一番上、赤字の文字、台帳のハッシュの写し取り、未確認が無いときの次の候補。

import { test, expect } from '@playwright/test';
import { open, apiCall, todayTokyo, walletOf, uniqueText, withSettings } from './helpers.mjs';

test('AC-07.1 最初に見えるのは「次にやること」で、数字の一覧より上にある', async ({ page }) => {
  await open(page, '#/');
  const next = page.locator('.next-card');
  await expect(next).toContainText('未確認の取引が');
  await expect(next.getByRole('link', { name: /未確認を開く/ })).toHaveAttribute('href', '#/inbox');
  const nextBox = await next.boundingBox();
  const kpiBox = await page.locator('.kpis').boundingBox();
  expect(nextBox.y + nextBox.height).toBeLessThanOrEqual(kpiBox.y);
  // 件数と出金額は、右（狭い幅では下）の大きな数字にも出す。数字と「円」は1つのかたまり。
  const dash = (await apiCall(page, 'GET', '/reports/dashboard')).json;
  const figures = next.locator('.next-figures');
  await expect(figures).toContainText(`未確認${dash.next_actions.inbox_draft.toLocaleString('en-US')}件`);
  await expect(figures).toContainText(`${dash.next_actions.inbox_out_amount.toLocaleString('en-US')}円`);
  await expect(figures.locator('.money').last()).toHaveCSS('white-space', 'nowrap');
});

test('SC-01 年の利益の補足は、期間・売上・経費を別の行に分ける（金額が途中で折れない）', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/?month=2026-09');
  const ytd = page.locator('.kpi', { hasText: '年の利益' });
  const rows = ytd.locator('.kpi-notes > div');
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(1)).toContainText('売上');
  await expect(rows.nth(2)).toContainText('経費');
  // 各行は1行に収まる（名前と金額が同じ高さにあり、金額が2行に割れない）。
  for (const i of [1, 2]) {
    const oneLine = await rows.nth(i).evaluate((el) => {
      const dt = el.querySelector('dt').getBoundingClientRect();
      const dd = el.querySelector('dd');
      const r = dd.getBoundingClientRect();
      const fs = parseFloat(getComputedStyle(dd).fontSize);
      return Math.abs(dt.top - r.top) < fs && r.height < fs * 2.2;
    });
    expect(oneLine).toBe(true);
  }
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
  // 色の決まり: 「赤字」の札は無彩色（朱は訂正・取消・失敗だけ。橙は保存前の注意だけ）。
  await expect(card.locator('.badge', { hasText: '赤字' })).toHaveClass(/badge--muted/);
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
  // 本文は利用者の言葉（記録は正常です）。ハッシュと写し取りは「くわしく」の中（2026-10-14 の画面の直し）。
  await expect(card).toContainText(`記録は正常です（${status.ledger.events.toLocaleString('en-US')}件）`);
  await expect(card.getByRole('button', { name: /写し取る/ })).toBeHidden();
  await card.locator('summary', { hasText: 'くわしく' }).click();
  await expect(card).toContainText(status.ledger.head_hash.slice(0, 8));
  await card.getByRole('button', { name: /写し取る/ }).click();
  await expect(page.locator('.toast').filter({ hasText: '写し取りました' })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(status.ledger.head_hash);

  await page.evaluate(() => {
    navigator.clipboard.writeText = () => Promise.reject(new Error('denied'));
  });
  await card.getByRole('button', { name: /写し取る/ }).click();
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

// SC-01 未確認は残高にも今月の数字にも入らない。口座の残高の見出しのすぐ下に短く書き、次にやることに未確認の説明を1行足す。
test('SC-01 未確認があれば、口座の残高の見出しの下に「残高に入っていない」と書き、次にやることで未確認を説明する', async ({ page }) => {
  await open(page, '#/');
  const dash = (await apiCall(page, 'GET', '/reports/dashboard')).json;
  test.skip(!dash.next_actions.inbox_draft, '未確認が無いデータ');
  const card = page.locator('section', { has: page.getByRole('heading', { name: '口座の残高' }) });
  const note = card.locator('.pending-note');
  await expect(note).toHaveText(`未確認 ${dash.next_actions.inbox_draft.toLocaleString('en-US')}件は、この残高に入っていません。`);
  // 見出しのすぐ下で、残高の行より上。
  const [head, line, rows] = await Promise.all([card.locator('.card-head').boundingBox(), note.boundingBox(), card.locator('.rows').boundingBox()]);
  expect(line.y).toBeGreaterThanOrEqual(head.y + head.height - 1);
  expect(line.y + line.height).toBeLessThanOrEqual(rows.y + 1);
  await expect(page.locator('.next-card')).toContainText('取り込んだ明細のうち、まだ確定していないものです。確定すると、ホームと帳票の数字に入ります。');
  // 0件なら、どちらも出さない。
  await page.route('**/api/v1/reports/dashboard*', async (route) => {
    const res = await route.fetch();
    const body = await res.json();
    body.next_actions = { inbox_draft: 0, inbox_out_amount: 0 };
    await route.fulfill({ response: res, json: body });
  });
  await page.reload();
  await expect(page.locator('.next-card')).toContainText('未確認はありません。');
  await expect(card.locator('.pending-note')).toHaveCount(0);
  await expect(page.locator('.next-card')).not.toContainText('まだ確定していないもの');
});

/** 2つの月（YYYY-MM）の間の月の数。 */
const monthsBetween = (a, b) => (Number(b.slice(0, 4)) - Number(a.slice(0, 4))) * 12 + Number(b.slice(5)) - Number(a.slice(5));

// SC-01 月の選択は、開業日の月から今月まで（開業前の月は記録できないので並べない）。今月より後でも、記録のある月は並べる。
test('SC-01 月の選択は開業日の月から今月までと、今月より後で記録のある月。開業日の月では前へ進めない', async ({ page }) => {
  await open(page, '#/');
  const first = (await apiCall(page, 'GET', '/settings')).json.opened_on.slice(0, 7);
  const thisMonth = todayTokyo().slice(0, 7);
  const values = () => page.locator('.month-select option').evaluateAll((os) => os.map((o) => o.value));
  let months = await values();
  expect(months.filter((m) => m < first), '開業日より前の月').toEqual([]);
  expect(months.filter((m) => m <= thisMonth)).toHaveLength(monthsBetween(first, thisMonth) + 1);
  expect(months[0] >= thisMonth).toBe(true);
  await page.goto(`/#/?month=${first}`);
  await expect(page.locator('.month-select')).toHaveValue(first);
  await expect(page.locator('.month-picker').getByRole('button', { name: '前の月' })).toBeDisabled();
  // 今年の、今月より後の月に記録を1つ作ると、その月も選べて、「次の月」で今月から進める（最後に取り消す）。
  test.skip(thisMonth.endsWith('-12'), '今月が12月だと、同じ年の後の月が無い');
  const later = `${thisMonth.slice(0, 4)}-${String(Number(thisMonth.slice(5)) + 1).padStart(2, '0')}`;
  const cash = await walletOf(page, 'cash');
  const made = await apiCall(page, 'POST', '/entries', { kind: 'expense', date: `${later}-15`, amount: 1200, account: 'exp_supplies', wallet: cash.code, tax_code: 'P10', partner: '', description: uniqueText('E2E 来月の記録'), business_pct: 100, memo: '' });
  expect(made.status, JSON.stringify(made.json)).toBe(201);
  try {
    await page.goto('/#/');
    await page.reload();
    await expect(page.locator('.kpis')).toBeVisible();
    months = await values();
    expect(months).toContain(later);
    await page.locator('.month-picker').getByRole('button', { name: '次の月' }).click();
    await expect(page).toHaveURL(new RegExp(`month=${later}`));
    await expect(page.locator('.month-select')).toHaveValue(later);
  } finally {
    const res = await apiCall(page, 'POST', `/entries/${made.json.id}/void`, { reason: 'E2E 後片付け', expected_revision: made.json.revision });
    expect(res.status, JSON.stringify(res.json)).toBe(200);
  }
});

// 月の途中の前月比（2026-10-15 の体験の評価 E1）: 10月15日に、9月の丸1か月と比べた「−500,500円」が下向きの矢印で出て、落ち込んだように見えた。
const monthLabel = (ym) => `${Number(ym.slice(5, 7))}月`;
const prevMonthOf = (ym) => {
  const [y, m] = ym.split('-').map(Number);
  return m === 1 ? `${y - 1}-12` : `${y}-${String(m - 1).padStart(2, '0')}`;
};

test('SC-01 今月（途中の月）のカードは、前月比の差と矢印を出さず、前の月の実額（1か月分）を添える。過去の月は今までどおり前月比', async ({ page }) => {
  await open(page, '#/');
  const thisMonth = todayTokyo().slice(0, 7);
  const prev = prevMonthOf(thisMonth);
  const dash = (await apiCall(page, 'GET', `/reports/dashboard?month=${thisMonth}`)).json;
  const cards = [
    ['売上', dash.month_summary.prev.revenue],
    ['経費', dash.month_summary.prev.expense],
    ['利益', dash.month_summary.prev.profit],
  ];
  for (const [what, amount] of cards) {
    const card = page.locator('.kpi', { hasText: `${monthLabel(thisMonth)}の${what}` });
    await expect(card.locator('.kpi-diff')).toHaveText(`${monthLabel(prev)}は ${amount.toLocaleString('en-US')}円（1か月分）`);
    await expect(card.locator('.kpi-diff svg')).toHaveCount(0);
  }
  await expect(page.locator('.kpis')).not.toContainText('前月比');
  // 過去の月（終わった月）は、今までどおり前月比と矢印。
  await page.goto(`/#/?month=${prev}`);
  const past = page.locator('.kpi', { hasText: `${monthLabel(prev)}の売上` });
  await expect(past.locator('.kpi-diff')).toContainText(/前月比|前月と同じ/);
  await expect(past.locator('.kpi-diff svg')).toHaveCount(1);
});

test('SC-01 途中の月で、前の月が開業前なら何も出さない。前の月が開業した月（1日でない）なら「1か月分」と書かない', async ({ page }) => {
  await open(page, '#/');
  const thisMonth = todayTokyo().slice(0, 7);
  const prev = prevMonthOf(thisMonth);
  await withSettings(page, { opened_on: `${thisMonth}-01` }, async () => {
    await page.reload();
    await expect(page.locator('.kpis')).toBeVisible();
    await expect(page.locator('.kpis .kpi-diff')).toHaveCount(0);
  });
  await withSettings(page, { opened_on: `${prev}-15` }, async () => {
    await page.reload();
    const card = page.locator('.kpi', { hasText: `${monthLabel(thisMonth)}の売上` });
    await expect(card.locator('.kpi-diff')).toHaveText(new RegExp(`^${monthLabel(prev)}は [-\\d,]+円$`));
  });
});

// ---- 見た目（2026-10-15 の見た目の採点） ----

test('SC-01 幅1440: 4枚の数字のカードの下の行（前の月の一言と、年の利益の内訳）は、カードの下の端にそろう', async ({ page }) => {
  await open(page, '#/');
  const bottoms = await page.locator('.kpis .kpi').evaluateAll((cards) =>
    cards.map((card) => {
      const last = card.lastElementChild.getBoundingClientRect();
      return Math.round(card.getBoundingClientRect().bottom - last.bottom);
    }),
  );
  expect(bottoms).toHaveLength(4);
  for (const b of bottoms) expect(Math.abs(b - bottoms[3])).toBeLessThanOrEqual(1);
});

test('SC-01 幅390: 前の月の一言は「（1か月分）」の途中で折れない。口座の名前は1行に入り、種類と金額は次の行', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/');
  const span = page.locator('.kpi-diff .keep').first();
  await expect(span).toHaveText('（1か月分）');
  const oneLine = (loc) =>
    loc.evaluate((el) => el.getBoundingClientRect().height < parseFloat(getComputedStyle(el).lineHeight) * 1.5);
  expect(await oneLine(span)).toBe(true);
  const rows = page.locator('.wallets-card .row');
  const n = await rows.count();
  expect(n).toBeGreaterThan(0);
  for (let i = 0; i < n; i += 1) {
    const r = rows.nth(i);
    expect(await oneLine(r.locator('.row-main > .keep'))).toBe(true);
    const [name, value] = await Promise.all([r.locator('.row-main > .keep').boundingBox(), r.locator('.row-value').boundingBox()]);
    expect(value.y).toBeGreaterThanOrEqual(name.y + name.height - 1);
  }
});

test('SC-01 開業日がその年の途中で、開業の月より前に記録が無ければ、年の利益の期間は開業の月から書く。記録があれば「1月〜」のまま', async ({ page }) => {
  const thisMonth = todayTokyo().slice(0, 7);
  const year = thisMonth.slice(0, 4);
  test.skip(thisMonth < `${year}-05`, '開業の月（4月）より後の月で確かめる');
  await open(page, '#/');
  const period = page.locator('.kpi', { hasText: '年の利益' }).locator('.kpi-period dd');
  await withSettings(page, { opened_on: `${year}-04-01` }, async () => {
    // 合成データは1月から記録があるので、集計どおり「1月〜」。
    await page.reload();
    await expect(period).toHaveText(`1月〜${monthLabel(thisMonth)}`);
    // 開業の月より前に記録が無い帳簿。ダッシュボードの応えの1〜3月を、月別の値と年の累計（ytd）の両方から外して見せる
    // （累計を元のままにすると、期間の文字だけを確かめても、金額との食い違いを見張れない。読み取りレビュー 2026-10-15）。
    const kpi = page.locator('.kpi', { hasText: '年の利益' });
    const before = `${year}-04`;
    const orig = (await apiCall(page, 'GET', '/reports/dashboard')).json;
    const gone = orig.series.filter((m) => m.month < before);
    const lose = (k) => gone.reduce((s, m) => s + m[k], 0);
    const ytd = { revenue: orig.ytd.revenue - lose('revenue'), expense: orig.ytd.expense - lose('expense'), profit: orig.ytd.profit - lose('profit') };
    await page.route('**/api/v1/reports/dashboard*', async (route) => {
      const res = await route.fetch();
      const body = await res.json();
      body.series = body.series.map((m) => (m.month < before ? { ...m, revenue: 0, expense: 0, profit: 0 } : m));
      body.ytd = { ...body.ytd, ...ytd };
      await route.fulfill({ response: res, json: body });
    });
    await page.reload();
    await expect(period).toHaveText(`4月〜${monthLabel(thisMonth)}`);
    // 期間の文字と金額が食い違わない: 売上・経費・利益の表示は、1〜3月を外した後の累計と同じ。
    for (const k of ['revenue', 'expense', 'profit']) await expect(kpi).toContainText(Math.abs(ytd[k]).toLocaleString('ja-JP'));
    // 売上も経費も入った月が、差し引き0円になっても、記録のある月として数える（記録の無い月は、売上も経費も0の月だけ）。
    await page.unroute('**/api/v1/reports/dashboard*');
    await page.route('**/api/v1/reports/dashboard*', async (route) => {
      const res = await route.fetch();
      const body = await res.json();
      body.series = body.series.map((m) => (m.month < before ? { ...m, revenue: m.month === `${year}-01` ? 100 : 0, expense: m.month === `${year}-01` ? 100 : 0, profit: 0 } : m));
      await route.fulfill({ response: res, json: body });
    });
    await page.reload();
    await expect(period).toHaveText(`1月〜${monthLabel(thisMonth)}`);
  });
});
