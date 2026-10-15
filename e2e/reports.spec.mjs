// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票（SC-06、AC-08.7）。6つのタブ、期間の変更（年・月・同じ年の中の自由な期間、貸借対照表は日付）、
// 選んだ期間がアドレスに入ること、各画面の「CSVで書き出す」のリンク（文字コードの選択つき）。

import { test, expect } from '@playwright/test';
import { open, withSettings, apiCall, walletOf, todayTokyo, newWallet, importRows, uniqueText } from './helpers.mjs';

const thisYear = new Date().getFullYear();
const TABS = [
  ['月次推移', 'monthly', 'income-statement.csv'],
  ['損益計算書', 'income-statement', 'income-statement.csv'],
  ['貸借対照表', 'balance-sheet', 'balance-sheet.csv'],
  ['試算表', 'trial-balance', 'trial-balance.csv'],
  ['総勘定元帳', 'ledger', 'ledger.csv'],
  ['消費税', 'tax-summary', 'tax-summary.csv'],
];
const exportLink = (page) => page.getByRole('link', { name: 'CSVで書き出す' });

test('AC-08.7 6つのタブを切り替えて表示でき、各画面にCSVで書き出すリンクがある', async ({ page }) => {
  await open(page, '#/reports');
  await expect(page).toHaveURL(/#\/reports\/monthly/);
  for (const [label, id, csv] of TABS) {
    await page.getByRole('tab', { name: label }).click();
    await expect(page).toHaveURL(new RegExp(`#/reports/${id}`));
    await expect(page.getByRole('tab', { name: label })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.report-title')).toContainText(label);
    if (id === 'ledger') {
      // 元帳は、科目を選ぶまで書き出せない。
      await expect(page.locator('.report-content')).toContainText('科目を選んでください');
      await expect(exportLink(page)).toHaveAttribute('aria-disabled', 'true');
      continue;
    }
    await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`/api/v1/export/${csv.replace('.', '\\.')}\\?.*encoding=utf8bom`));
    if (id === 'trial-balance') {
      // 試算表は、期首・借方・貸方・期末の4つの数字の列（設計レビューで加わった条件）。
      await expect(page.getByRole('table', { name: '試算表' }).locator('thead th')).toHaveText(['科目', '期首', '借方', '貸方', '期末']);
    }
  }
  await page.getByRole('tab', { name: '月次推移' }).click();
  await expect(page.locator('.report-content svg[role="img"]')).toHaveAttribute('aria-label', /推移/);
  await expect(page.getByRole('table', { name: /月次推移（(?:グラフと同じ数字|今月より後の月は、表だけに出ます)）/ })).toBeVisible();
});

test('AC-08.7 期間を年・月・自由な期間で変えられ、アドレスに入る。年をまたぐ期間は、選ぶ時点で止める', async ({ page }) => {
  await open(page, '#/reports/income-statement');
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年`);
  // 年: [ キー（前の年のボタンと同じ）と、選択欄。選択欄の年は開業日の年からなので、前の年へは [ で移る
  // （開業日が今年の帳簿でも動くように）。
  await page.locator('main h1').focus();
  await page.keyboard.press('[');
  await expect(page).toHaveURL(new RegExp(`year=${thisYear - 1}`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear - 1}年`);
  // 戻った先の年が、年の候補（開業日の年から）に無くても、選択欄は空にならず、その年を出す（体験の評価 第3回 E3）。
  const yearSel = page.getByRole('combobox', { name: '年', exact: true });
  await expect(yearSel).toHaveValue(String(thisYear - 1));
  await page.keyboard.press('[');
  await expect(page).toHaveURL(new RegExp(`year=${thisYear - 2}`));
  await expect(yearSel).toHaveValue(String(thisYear - 2));
  await page.getByRole('combobox', { name: '年', exact: true }).selectOption(String(thisYear));
  await expect(page).toHaveURL(new RegExp(`year=${thisYear}`));
  // 月。
  await page.locator('.seg-option', { hasText: '月' }).first().click();
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-\\d{2}`));
  await page.getByRole('combobox', { name: '月', exact: true }).selectOption('03');
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-03`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年3月`);
  // 自由な期間（同じ年の中）。
  await page.locator('.seg-option', { hasText: '自由な期間' }).click();
  const from = page.getByLabel('期間の始め');
  const to = page.getByLabel('期間の終わり');
  await from.fill('0401');
  await from.press('Tab');
  await to.fill('0630');
  await to.press('Tab');
  await page.getByRole('button', { name: 'この期間で見る' }).click();
  await expect(page).toHaveURL(new RegExp(`from=${thisYear}-04-01&to=${thisYear}-06-30`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}-04-01 〜 ${thisYear}-06-30`);
  // 年をまたぐ日付は受け付けず、理由を出して問い合わせない。押しても、直す欄（終わり）へフォーカスを戻すだけ
  // （押せないボタンにしていたときは、押した瞬間にフォーカスが本文の外へ落ちた。体験の評価 第2回 E3-3）。
  await to.fill(`${thisYear + 1}-01-31`);
  await to.press('Tab');
  await expect(page.locator('.period-controls .field-error')).toContainText(`${thisYear}年の中の日付にしてください`);
  await expect(to).toHaveAttribute('aria-invalid', 'true');
  await page.getByRole('button', { name: 'この期間で見る' }).click();
  await expect(to).toBeFocused();
  await expect(page).toHaveURL(new RegExp(`to=${thisYear}-06-30`));
  // 選んだ期間は、CSVのリンクにも入る。
  await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`from=${thisYear}-04-01&to=${thisYear}-06-30`));
  // 戻る操作で、前の期間に戻る。
  await page.goBack();
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-03`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年3月`);
});

test('AC-08.7 貸借対照表は日付を1つ選ぶ。当期純利益の行があり、資産と負債・資本が一致する', async ({ page }) => {
  await open(page, '#/reports/balance-sheet');
  const date = page.getByLabel('いつの時点か');
  await date.fill(`${thisYear}0630`);
  await date.press('Enter');
  await date.press('Tab');
  await expect(page).toHaveURL(new RegExp(`as_of=${thisYear}-06-30`));
  await expect(page.locator('.report-title')).toHaveText(`貸借対照表　${thisYear}-06-30 時点`);
  await expect(page.locator('.report-content')).toContainText('当期純利益');
  await expect(page.locator('.report-check')).toContainText('一致しています');
  await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`/export/balance-sheet\\.csv\\?as_of=${thisYear}-06-30`));
});

test('AC-08.6 の画面側: 文字コードを Shift_JIS にするとリンクが変わり、書き出したCSVを受け取れる（既定はBOMつきUTF-8）', async ({ page }) => {
  await open(page, '#/reports/trial-balance');
  const href = await exportLink(page).getAttribute('href');
  const res = await page.request.get(new URL(href, page.url()).href);
  expect(res.status()).toBe(200);
  expect(res.headers()['content-type']).toContain('text/csv');
  const bytes = await res.body();
  expect([...bytes.subarray(0, 3)]).toEqual([0xef, 0xbb, 0xbf]);
  await page.getByLabel('CSVの文字コード').selectOption('sjis');
  await expect(exportLink(page)).toHaveAttribute('href', /encoding=sjis/);
  const sj = await page.request.get(new URL(await exportLink(page).getAttribute('href'), page.url()).href);
  expect(sj.status()).toBe(200);
  expect([...(await sj.body()).subarray(0, 3)]).not.toEqual([0xef, 0xbb, 0xbf]);
  await page.getByLabel('CSVの文字コード').selectOption('utf8bom');
});

test('SC-06 総勘定元帳: 科目を選ぶと行と残高の推移が出て、行から仕訳の詳細に飛べる', async ({ page }) => {
  await open(page, '#/reports/ledger');
  const acc = page.getByRole('combobox', { name: '科目' });
  await acc.click();
  await page.keyboard.insertText('げんきん');
  // 口座が先（Enter で選ばれる先頭が、動きのある口座）。同じ名前の親の科目「現金」は、直接の記帳しか出ず、0行になる
  // （体験の評価 第3回 E3）。
  await expect(page.getByRole('option', { name: /現金/ }).first()).toContainText('口座');
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/account=/);
  const rows = page.locator('.ledger-table tbody tr');
  await expect(rows.first()).toBeVisible();
  await expect(page.locator('.ledger-facts')).toContainText('期首の残高');
  await expect(exportLink(page)).toHaveAttribute('href', /\/export\/ledger\.csv\?account=/);
  await rows.first().focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal\/e_/);
  await expect(page.getByRole('dialog')).toContainText('の仕訳');
});

test('SC-06 消費税: 上に「参考値」の注意と表の最終確認日。免税の設定では冒頭に案内を出す', async ({ page }) => {
  await open(page, '#/reports/tax-summary');
  const content = page.locator('.report-content');
  await expect(content).toContainText('参考値。簡易課税・特例は反映していません');
  await expect(content).toContainText(/経過措置の表の最終確認日は\d{4}-\d{2}-\d{2}です。/);
  await expect(content).toContainText('納付額の見込み');
  await withSettings(page, { tax_mode: 'exempt', invoice_number: null }, async () => {
    await page.reload();
    const first = page.locator('.report-content .banner').first();
    await expect(first).toContainText('設定上は免税事業者として表示しています。申告の要否は、国税庁の案内か税理士に確かめてください');
  });
});

/** 帳票の表を、見出しと、行ごとの名前と数字（「—」は null）にして読む。 */
function readReportTable(table) {
  return table.evaluate((t) => {
    const num = (td) => {
      const s = td.textContent.replace(/なし|—/g, '').replace(/[,\s]/g, '');
      return s === '' ? null : Number(s);
    };
    const parents = [];
    return {
      head: [...t.querySelectorAll('thead th')].map((th) => th.textContent.trim()),
      rows: [...t.querySelectorAll('tbody tr')]
        .filter((tr) => tr.querySelector('td') && !tr.classList.contains('rt-fill'))
        .map((tr) => {
          const name = tr.querySelector('.rt-name-text').textContent.trim();
          const depth = Number(tr.style.getPropertyValue('--depth'));
          if (tr.classList.contains('rt-parent')) parents[depth] = name;
          const directParent = tr.querySelector('.is-direct') ? parents[depth - 1] : null;
          return { name, directParent, cells: [...tr.querySelectorAll('td')].map(num) };
        }),
    };
  });
}

// AC-08.3 の画面側（12か月の合計が年の合計と一致する）。今月より後でも金額のある月は列に出し、列を足すと右端の「合計」と合う。
// 振替だけの月（収益と費用の合計が0）でも、科目の金額は「—」にしない。仕訳は残ってよい（段階ごとに新しい帳簿で始まる）。
test('AC-08.3 月次推移の月別値と年間合計はAPIと一致し、来月と振替だけの月も科目の金額を出す', async ({ page }) => {
  await open(page, '#/');
  const monthAfter = (n) => {
    const d = new Date(`${todayTokyo().slice(0, 7)}-01T00:00:00Z`);
    d.setUTCMonth(d.getUTCMonth() + n);
    return d.toISOString().slice(0, 7);
  };
  const [next, after] = [monthAfter(1), monthAfter(2)];
  const cash = await walletOf(page, 'cash');
  const a = await apiCall(page, 'POST', '/entries', { kind: 'expense', date: `${next}-10`, amount: 1234, account: 'exp_comm', wallet: cash.code, tax_code: 'P10', partner: '', description: '来月の取引（試験）', business_pct: 100, memo: '' });
  expect(a.status).toBe(201);
  const b = await apiCall(page, 'POST', '/entries', {
    kind: 'journal',
    date: `${after}-10`,
    description: '費用どうしの振替（試験）',
    partner: '',
    lines: [
      { account: 'exp_comm', side: 'debit', amount: 10000, tax_code: null, memo: '' },
      { account: 'exp_supplies', side: 'credit', amount: 10000, tax_code: null, memo: '' },
    ],
  });
  expect(b.status).toBe(201);
  const monthLabel = (ym) => `${Number(ym.slice(5, 7))}月`;
  for (const year of new Set([next.slice(0, 4), after.slice(0, 4)])) {
    await page.goto(`/#/reports/monthly?year=${year}`);
    await expect(page.locator('.report-title')).toHaveText(`月次推移　${year}年`);
    const response = await apiCall(page, 'GET', `/reports/income-statement?from=${year}-01-01&to=${year}-12-31&by=month`);
    expect(response.status).toBe(200);
    const report = response.json;
    const raw = [...report.revenue.rows, ...report.expense.rows];
    const expectedMonths = report.months.filter((m, i) => m.month <= todayTokyo().slice(0, 7) || raw.some((r) => r.months[i] !== 0));
    const future = expectedMonths.some((m) => m.month > todayTokyo().slice(0, 7));
    const caption = `${year}年の月次推移（${future ? '今月より後の月は、表だけに出ます' : 'グラフと同じ数字'}）`;
    const table = page.getByRole('table', { name: caption, exact: true });
    await expect(table).toBeVisible();
    const sum = await readReportTable(table);
    const grand = sum.rows.find((r) => r.name === '合計');
    const months = sum.rows.filter((r) => r !== grand);
    expect(months.map((r) => r.name)).toEqual(expectedMonths.map((m) => monthLabel(m.month)));
    for (const m of expectedMonths) {
      const values = !m.revenue && !m.expense ? [null, null, null] : [m.revenue, m.expense, m.net_income];
      expect(months.find((r) => r.name === monthLabel(m.month)).cells, `${m.month} の API 値`).toEqual(values);
    }
    expect(grand.cells).toEqual([report.revenue.total, report.expense.total, report.net_income]);
    for (let k = 0; k < 3; k += 1) expect(months.reduce((x, r) => x + (r.cells[k] || 0), 0), `月ごとの数字の${sum.head[k + 1]}`).toBe(grand.cells[k]);
    // 科目ごとの月別: 各行の月の列を足すと、右端の「合計」と合う。
    await page.getByText('科目ごとの月別を見る').click();
    const by = await readReportTable(page.getByRole('table', { name: `${year}年の科目ごとの月別` }));
    const columns = expectedMonths.map((m) => report.months.findIndex((x) => x.month === m.month));
    const monthValues = (values) => columns.map((i) => raw.some((r) => r.months[i] !== 0) ? values[i] : null);
    for (const [name, sec] of [['収益', report.revenue], ['費用', report.expense]]) {
      const values = report.months.map((_, i) => sec.rows.reduce((n, r) => n + r.months[i], 0));
      expect(by.rows.find((r) => r.name === `${name}の合計`).cells).toEqual([...monthValues(values), sec.total]);
    }
    expect(by.rows.find((r) => r.name === '当期純利益').cells).toEqual([...monthValues(report.months.map((m) => m.net_income)), report.net_income]);
    // 親の行は子を含むので、API の平らな科目の値を親へ足して比較する。
    const accounts = await apiCall(page, 'GET', '/accounts?include_disabled=1');
    expect(accounts.status).toBe(200);
    const catalog = new Map(accounts.json.items.map((a) => [a.code, a]));
    const totals = new Map();
    for (const r of raw) {
      for (let code = r.code; code; code = catalog.get(code)?.parent) {
        const value = totals.get(code) || { amount: 0, months: report.months.map(() => 0) };
        value.amount += r.amount;
        r.months.forEach((v, i) => { value.months[i] += v; });
        totals.set(code, value);
      }
    }
    const totalNames = new Set(['収益の合計', '費用の合計', '当期純利益']);
    for (const r of by.rows.filter((r) => !totalNames.has(r.name))) {
      const account = accounts.json.items.find((a) => a.name === (r.directParent || r.name));
      expect(account, `${r.name} の科目`).toBeTruthy();
      const value = r.directParent ? raw.find((x) => x.code === account.code) : totals.get(account.code);
      expect(value, `${r.name} の API 値`).toBeTruthy();
      expect(r.cells, `${r.name} の月別・年間値`).toEqual([...monthValues(value.months), value.amount]);
    }
    for (const r of by.rows) expect(r.cells.slice(0, -1).reduce((x, v) => x + (v || 0), 0), `${r.name} の月の列の合計`).toBe(r.cells.at(-1));
    // 月の列と、上の表の月は同じ。
    expect(by.head.slice(1, -1)).toEqual(months.map((r) => r.name));
    if (next.startsWith(year)) {
      expect(by.head).toContain(monthLabel(next));
      const i = by.head.indexOf(monthLabel(next)) - 1;
      expect(by.rows.find((r) => r.name === '通信費').cells[i]).toBeGreaterThanOrEqual(1234);
    }
    if (after.startsWith(year)) {
      expect(by.head).toContain(monthLabel(after));
      const i = by.head.indexOf(monthLabel(after)) - 1;
      // 振替だけの月: 上の表は「—」、科目の表は実際の金額。
      expect(months.find((r) => r.name === monthLabel(after)).cells).toEqual([null, null, null]);
      expect(by.rows.find((r) => r.name === '通信費').cells[i]).toBe(10000);
      expect(by.rows.find((r) => r.name === '消耗品費').cells[i]).toBe(-10000);
      expect(by.rows.find((r) => r.name === '費用の合計').cells[i]).toBe(0);
    }
  }
});

/** 表の形を測る: 行の表示（カードなら flex）、見えている見出しの数、枠に収まっているか。 */
function tableShape(table) {
  return table.evaluate((t) => {
    const wrap = t.closest('.table-wrap');
    const row = [...t.querySelectorAll('tbody tr')].find((tr) => tr.querySelector('td') && !tr.classList.contains('rt-fill'));
    // カードのときの見出しは、1px に切り抜いて読み上げにだけ残す（中の th の幅では見分けられない）。
    const headShown = t.querySelector('thead').getBoundingClientRect().height > 2;
    const heads = headShown ? [...t.querySelectorAll('thead th')].filter((th) => getComputedStyle(th).display !== 'none') : [];
    const label = getComputedStyle(row.querySelector('td'), '::before').content;
    return { rowDisplay: getComputedStyle(row).display, heads: heads.map((th) => th.textContent.trim()), fits: wrap.scrollWidth <= wrap.clientWidth + 1, label };
  });
}

// AC-11.6 表は幅に合わせて列を畳む: 幅390の試算表と月次推移は1行を1枚のカードに、幅768の総勘定元帳は5列の表で枠に収まる。
test('AC-11.6 幅390では試算表と月次推移がカード、幅768では元帳が5列の表で枠に収まる', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/reports/trial-balance');
  // 表の形で枠に入らない表だけが、カードになる（形が決まると、枠に data-fit が付く）。
  await expect(page.getByRole('region', { name: '試算表' })).toHaveAttribute('data-fit', 'cards');
  const trial = await tableShape(page.getByRole('table', { name: '試算表' }));
  expect(trial.rowDisplay).toBe('flex');
  expect(trial.heads).toEqual([]);
  expect(trial.fits).toBe(true);
  // カードの数字には、列の名前（期首・借方・貸方・期末）が付く。
  expect(trial.label).not.toBe('none');
  await expect(page.getByRole('table', { name: '試算表' }).locator('tbody tr:not(.rt-group)').first().locator('td').first()).toHaveAttribute('data-label', '期首');
  await page.getByRole('tab', { name: '月次推移' }).click();
  await expect(page.getByRole('region', { name: /月次推移（(?:グラフと同じ数字|今月より後の月は、表だけに出ます)）/ })).toHaveAttribute('data-fit', 'cards');
  const monthly = await tableShape(page.getByRole('table', { name: /月次推移（(?:グラフと同じ数字|今月より後の月は、表だけに出ます)）/ }));
  expect(monthly.rowDisplay).toBe('flex');
  expect(monthly.fits).toBe(true);

  await page.setViewportSize({ width: 768, height: 1024 });
  const w = await walletOf(page, 'card');
  await page.goto(`/#/reports/ledger?account=${w.code}`);
  await expect(page.locator('.ledger-table tbody tr').first()).toBeVisible();
  const ledger = await tableShape(page.locator('.ledger-table'));
  expect(ledger.rowDisplay).toBe('table-row');
  // No と相手科目は摘要の2行目に畳み、日付・摘要・借方・貸方・残高の5列。
  expect(ledger.heads).toEqual(['日付', '摘要', '借方', '貸方', '残高']);
  expect(ledger.fits).toBe(true);
  await expect(page.locator('.ledger-table tbody tr').first().locator('.lg-sub')).toBeVisible();
  // 同じ幅の試算表は、表のまま（カードにしない）。
  await page.getByRole('tab', { name: '試算表' }).click();
  await expect(page.getByRole('region', { name: '試算表' })).toHaveAttribute('data-fit', 'table');
  const wide = await tableShape(page.getByRole('table', { name: '試算表' }));
  expect(wide.rowDisplay).toBe('table-row');
  expect(wide.heads).toEqual(['科目', '期首', '借方', '貸方', '期末']);
  expect(wide.fits).toBe(true);
  expect(wide.label).toBe('none');
});

// AC-11.6 横に動かしてよいのは、帳票の切替の帯だけ（nfr.md）。狭い幅では、選んだタブを帯の中の見える位置に寄せる。
test('AC-11.6 帳票のタブの帯だけが横に動き、選んだタブは幅390でも帯の中に見えている', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  for (const [label, id] of [['消費税', 'tax-summary'], ['総勘定元帳', 'ledger'], ['月次推移', 'monthly']]) {
    await open(page, `#/reports/${id}`);
    await expect(page.getByRole('tab', { name: label })).toHaveAttribute('aria-selected', 'true');
    const strip = await page.evaluate(() => {
      const marked = [...document.querySelectorAll('[data-allow-hscroll]')];
      const bar = document.querySelector('.report-tabs');
      const sel = bar.querySelector('[aria-selected="true"]').getBoundingClientRect();
      const box = bar.getBoundingClientRect();
      return { marked: marked.map((e) => e.getAttribute('role')), scrolls: bar.scrollWidth > bar.clientWidth, inside: sel.left >= box.left - 1 && sel.right <= box.right + 1 };
    });
    expect(strip.marked).toEqual(['tablist']);
    // 幅390では、6つのタブが帯に入りきらない（横に動く帯である）。
    expect(strip.scrolls).toBe(true);
    expect(strip.inside, `${label} のタブが帯の外にある`).toBe(true);
  }
  // 帳票の外の画面には、横に動かしてよい印が無い。
  for (const hash of ['#/', '#/journal', '#/wallets']) {
    await open(page, hash);
    expect(await page.locator('[data-allow-hscroll]').count()).toBe(0);
  }
});

/** 帳票の中で、数字が枠から横にはみ出している要素と、横に隠れる列を数える（読み上げ用に隠した文字は数えない）。 */
function overflowsIn(page) {
  return page.evaluate(() => {
    const out = [];
    for (const el of document.querySelectorAll('main td, main th, main dd, main dt')) {
      const r = el.getBoundingClientRect();
      if (!r.width || !r.height) continue;
      let left = Infinity;
      let right = -Infinity;
      const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if (!n.textContent.trim() || n.parentElement.closest('.visually-hidden')) continue;
        const range = document.createRange();
        range.selectNodeContents(n);
        const b = range.getBoundingClientRect();
        if (!b.width) continue;
        left = Math.min(left, b.left);
        right = Math.max(right, b.right);
      }
      if (right > r.right + 1 || left < r.left - 1) out.push(`${el.tagName.toLowerCase()}「${el.textContent.trim().slice(0, 16)}」`);
    }
    for (const e of document.querySelectorAll('main *')) {
      const b = e.getBoundingClientRect();
      if (/(auto|scroll)/.test(getComputedStyle(e).overflowX) && e.scrollWidth > e.clientWidth + 1 && b.width > 0 && !e.closest('[data-allow-hscroll]')) {
        out.push(`隠れる列 ${String(e.className)} ${e.scrollWidth}>${e.clientWidth}`);
      }
    }
    if (document.documentElement.scrollWidth > document.documentElement.clientWidth) out.push('ページが横に広がる');
    return out;
  });
}

// AC-11.6（設計担当の追加の指摘 4）: 10億円を超える金額（負の利益も）でも、帳票のカードと結論の数字は隣の升目に重ならず、
// 枠から横にはみ出さない。表は、入りきらなければカードに積み上げる（列を隠さない）。仕訳は残ってよい（この段階の最後の試験）。
// 総勘定元帳は幅640〜1023pxを除く。9桁までは収まるが、10桁以上は金額の3列が枠に入らず列が隠れる既知の限界があるため。
test('AC-11.6 10億円を超える金額でも帳票のカードと数字は重ならず列が隠れない（元帳は幅640〜1023pxを除く）', async ({ page }) => {
  await open(page, '#/');
  const year = todayTokyo().slice(0, 4);
  const bank = await walletOf(page, 'bank');
  const card = await walletOf(page, 'card');
  const big = [
    { kind: 'income', date: `${year}-01-20`, amount: 1234567890, account: 'sales', wallet: bank.code, tax_code: 'S10' },
    { kind: 'expense', date: `${year}-02-20`, amount: 1500000000, account: 'exp_outsource', wallet: bank.code, tax_code: 'P10' },
    { kind: 'expense', date: `${year}-03-20`, amount: 987654321, account: 'exp_supplies', wallet: card.code, tax_code: 'P10X' },
  ];
  for (const b of big) {
    const res = await apiCall(page, 'POST', '/entries', { ...b, partner: '', description: '大きな金額（試験）', business_pct: 100, memo: '' });
    expect(res.status).toBe(201);
  }
  const ledger = `#/reports/ledger?account=${bank.code}&month=${year}-02`;
  for (const [width, height] of [[390, 844], [768, 1024], [1440, 900]]) {
    await page.setViewportSize({ width, height });
    const hashes = ['#/reports/monthly', '#/reports/income-statement', '#/reports/balance-sheet', '#/reports/trial-balance', '#/reports/tax-summary'];
    if (width < 640 || width >= 1024) hashes.push(ledger);
    for (const hash of hashes) {
      await page.goto(`/${hash}`);
      await expect(page.locator('.report-content .report-body')).toBeVisible();
      await page.evaluate(() => document.querySelectorAll('main details').forEach((d) => { d.open = true; }));
      // カードか表かが決まるまで待つ（開閉の中の表も）。
      await expect.poll(() => page.evaluate(() => [...document.querySelectorAll('.rt-sizer')].every((s) => s.parentElement.dataset.fit))).toBe(true);
      expect(await overflowsIn(page), `${hash} 幅${width}`).toEqual([]);
    }
  }
});

// SC-06 未確認（取り込んで、まだ確定していない明細）は、どの帳票にも入らない。6つの帳票の期間の下に、その期間の未確認の
// 件数と出金・入金の合計を書き、未確認へのリンクを置く。口座の元帳は、その口座の未確認だけ。0件なら出さない。
test('SC-06 帳票の期間の下に、集計に入っていない未確認の件数と金額を書く。0件なら出さない', async ({ page }) => {
  await open(page, '#/');
  const today = todayTokyo();
  const year = today.slice(0, 4);
  const w = await newWallet(page, 'bank', uniqueText('E2E 未確認の注記'));
  await importRows(page, w.code, [[today, 'E2E 注記の出金', '1650', ''], [today, 'E2E 注記の入金', '', '30000']]);
  const expected = async (params) => {
    const q = new URLSearchParams({ status: 'draft', limit: '500', ...params });
    const res = (await apiCall(page, 'GET', `/drafts?${q}`)).json;
    expect(res.total, '試験の前提: 500件以下').toBeLessThanOrEqual(500);
    const sum = (dir) => res.items.filter((d) => d.direction === dir).reduce((a, d) => a + d.amount, 0).toLocaleString('en-US');
    return `未確認 ${res.total.toLocaleString('en-US')}件（出金 ${sum('out')}円・入金 ${sum('in')}円）は、この集計に入っていません。`;
  };
  const yearText = await expected({ from: `${year}-01-01`, to: `${year}-12-31` });
  const note = page.locator('.report-pending');
  for (const [label, id] of TABS.filter(([, x]) => x !== 'ledger')) {
    await page.goto(`/#/reports/${id}`);
    await expect(page.locator('.report-title')).toContainText(label);
    await expect(note, label).toHaveText(`${id === 'balance-sheet' ? await expected({ to: today }) : yearText} 未確認を開く`);
    await expect(note.getByRole('link', { name: '未確認を開く' })).toHaveAttribute('href', '#/inbox');
    // 期間の欄のすぐ下（帳票の見出しより上）。
    const [bar, line, head] = await Promise.all([page.locator('.report-toolbar').boundingBox(), note.boundingBox(), page.locator('.report-title').boundingBox()]);
    expect(line.y, label).toBeGreaterThanOrEqual(bar.y + bar.height - 1);
    expect(line.y + line.height, label).toBeLessThanOrEqual(head.y + 1);
  }
  // 口座の元帳は、その口座の未確認だけを数え、リンクもその口座で絞った未確認へ。
  await page.goto(`/#/reports/ledger?account=${w.code}`);
  await expect(page.locator('.report-title')).toContainText('総勘定元帳');
  await expect(note).toHaveText('未確認 2件（出金 1,650円・入金 30,000円）は、この集計に入っていません。 未確認を開く');
  await expect(note.getByRole('link', { name: '未確認を開く' })).toHaveAttribute('href', `#/inbox?wallet=${w.code}`);
  // 0件なら出さない。
  await page.route('**/api/v1/drafts?*', (route) => route.fulfill({ json: { items: [], total: 0, counts: { draft: 0, ignored: 0, confirmed: 0 } } }));
  await page.goto('/#/reports/income-statement');
  await expect(page.locator('.report-title')).toContainText('損益計算書');
  await expect(note).toBeHidden();
  await page.unroute('**/api/v1/drafts?*');
});

// SC-06 消費税: 「参考値」の注意の帯は、どの幅でも1行の要点だけにし、本文と言葉の説明は「くわしく」に畳む。答え（納付額の見込み）は
// 最初の画面（狭い幅は下のバーより上）に入る（見た目の採点 第2回: 幅1440でも、結論より先に全幅の注意枠が目に入った）。
// 売上税額・仕入税額が、税込の合計から計算し直した額と違う理由（明細ごとの税額の合計）を書く（体験の評価 第2回 E3-12）。
test('SC-06 消費税は、どの幅でも注意を「くわしく」に畳み、納付額の見込みが最初の画面に入る', async ({ page }) => {
  await open(page, '#/reports/tax-summary');
  for (const [width, height] of [[390, 844], [768, 900], [1024, 900], [1440, 900]]) {
    // 幅ごとに読み込み直す（同じアドレスへの goto では作り直されず、前の幅で作った画面のままになる）。
    await page.setViewportSize({ width, height });
    await page.reload();
    const more = page.locator('.tax-more');
    await expect(page.locator('.report-content')).toContainText('参考値。簡易課税・特例は反映していません');
    await expect(more, `幅${width}`).not.toHaveAttribute('open', /.*/);
    await expect(page.locator('.tax-more-body'), `幅${width}`).toBeHidden();
    await expect(more.locator('summary'), `幅${width}`).toBeVisible();
    const main = page.locator('.tax-sums .rt-figure.is-main');
    await expect(main).toContainText('納付額の見込み');
    const bottom = width < 768 ? await page.locator('.bottomnav').evaluate((el) => el.getBoundingClientRect().top) : height;
    const [box, scrolled] = await Promise.all([main.boundingBox(), page.evaluate(() => window.scrollY)]);
    expect(scrolled).toBe(0);
    expect(box.y + box.height, `幅${width}: 納付額の見込みの下の端 ${box.y + box.height}px と、最初の画面の下の端 ${bottom}px`).toBeLessThanOrEqual(bottom);
  }
  // 「くわしく」で、本文と言葉の説明を開ける。
  await page.locator('.tax-more > summary').click();
  await expect(page.locator('.tax-more-body')).toBeVisible();
  await expect(page.locator('.tax-more-body')).toContainText(/経過措置の表の最終確認日は\d{4}-\d{2}-\d{2}です。/);
  await expect(page.locator('.report-content .report-note')).toHaveText('売上税額は、明細ごとの税額の合計です。仕入税額は、通常の仕入れは明細ごとの税額の合計、経過措置の仕入れは、税額に控除できる割合を掛けた額の合計です。どちらも、税込の合計から計算し直すと、1円未満の処理の分だけ違うことがあります。');
});

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

// SC-06 総勘定元帳（体験の評価 第2回 E3-1・E3-10・E4-13・E4-14）: 口座をまとめる科目（普通預金・その他の預金）を選ぶと
// 0行で「動きがありません」と出て、預金の出入りが無いように見えた（動きは口座の側にある）。一言と、口座ごとの元帳へのリンク
// （同じ期間）を出す。口座は種類の言葉（よきん・ふつう）でも探せる。合う候補が無ければ、そう書く。見出し（知らせ）に科目の名前。
// 行には仕訳の id（担当をまたぐ約束 C1。詳細を閉じたときに、開いた行へ戻すため）。j・k で行を動かす（仕訳帳と同じ）。
test('SC-06 総勘定元帳: 口座をまとめる科目では口座の元帳へ同じ期間でつなぎ、口座は種類の言葉でも探せる。行には仕訳の id', async ({ page }) => {
  await open(page, '#/reports/ledger');
  const year = todayTokyo().slice(0, 4);
  const accounts = (await apiCall(page, 'GET', '/accounts?include_disabled=1')).json.items;
  const bank = accounts.find((a) => a.wallet && a.wallet.type === 'bank' && a.enabled && a.parent === 'dep_other');
  const kids = accounts.filter((a) => a.parent === 'dep_other').sort((a, b) => a.sort - b.sort || (a.code < b.code ? -1 : 1));
  expect(kids.every((a) => a.wallet), '試験の前提: 普通預金の下は口座だけ').toBe(true);
  const acc = page.getByRole('combobox', { name: '科目' });
  for (const word of ['よきん', 'ふつう']) {
    await acc.fill('');
    await acc.click();
    await page.keyboard.insertText(word);
    await expect(page.getByRole('option', { name: new RegExp(`^${esc(bank.name)}`) }), word).toBeVisible();
  }
  await page.getByRole('option', { name: /^普通預金・その他の預金/ }).click();
  await expect(page).toHaveURL(new RegExp(`year=${year}&account=dep_other`));
  await expect(page.locator('.report-title')).toHaveText(`総勘定元帳（普通預金・その他の預金）　${year}年`);
  const direct = (await apiCall(page, 'GET', `/reports/ledger?account=dep_other&from=${year}-01-01&to=${year}-12-31`)).json.rows.length > 0;
  const names = kids.map((a) => `「${a.name}」`).join('');
  await expect(page.locator('.ledger-children-text')).toHaveText(direct ? `この表は、この科目への直接の記帳だけです。口座${names}の動きは、${kids.length > 1 ? 'それぞれの' : 'その'}口座の元帳にあります。` : `この科目の動きは、口座${names}の側にあります。`);
  const links = page.locator('.ledger-children a');
  expect(await links.evaluateAll((as) => as.map((a) => a.getAttribute('href')))).toEqual(kids.map((a) => `#/reports/ledger?year=${year}&account=${a.code}`));
  // 月で見ているときは、リンクの先も同じ月。キーボードで口座の元帳へ移ると、フォーカスは本文の外に落ちず、表の行に着く。
  await page.goto(`/#/reports/ledger?month=${year}-09&account=dep_other`);
  const link = page.getByRole('link', { name: `${bank.name}の元帳` });
  await expect(link).toHaveAttribute('href', `#/reports/ledger?month=${year}-09&account=${bank.code}`);
  await link.focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(new RegExp(`month=${year}-09&account=${bank.code}`));
  await expect(page.locator('.report-title')).toHaveText(`総勘定元帳（${bank.name}）　${year}年9月`);
  const rows = page.locator('.ledger-table tbody tr');
  await expect(rows.first()).toBeFocused();
  // 行の印（約束 C1）: 各行の data-entry-id は、その行の仕訳の id。
  const api = (await apiCall(page, 'GET', `/reports/ledger?account=${bank.code}&from=${year}-09-01&to=${year}-09-30`)).json.rows;
  expect(api.length, '試験の前提: 9月に2行以上').toBeGreaterThan(1);
  expect(await rows.evaluateAll((trs) => trs.map((tr) => tr.getAttribute('data-entry-id')))).toEqual(api.map((r) => r.entry_id));
  // j・k で行を動かす。
  await page.keyboard.press('j');
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press('k');
  await expect(rows.first()).toBeFocused();
  // 合う候補が無いときは、一覧を開かず、欄の下に一言（読み上げにも伝える）。
  await acc.fill('');
  await acc.click();
  await page.keyboard.insertText('ずずず');
  await expect(page.locator('.combo-msg')).toHaveText('合う科目がありません。ひらがなの読みか、科目の名前で探せます。');
  await expect(acc).toHaveAttribute('aria-expanded', 'false');
});

// AC-08.7 SC-06 自由な期間の誤り（体験の評価 第2回 E3-3）: 始めが終わりより後なら、誤りを始めの欄に結び付け（印と説明）、
// フォーカスをその欄へ移す。誤りのあいだは、表がどの期間のままかを見出しの横に書く。打ち直したら、押し直す前に誤りを消す。
test('AC-08.7 自由な期間で始めが終わりより後なら、誤りを始めの欄に結び付けてフォーカスを移し、直したら消える', async ({ page }) => {
  const y = thisYear;
  await open(page, `#/reports/income-statement?from=${y}-04-01&to=${y}-06-30`);
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${y}-04-01 〜 ${y}-06-30`);
  const from = page.getByLabel('期間の始め');
  const to = page.getByLabel('期間の終わり');
  const apply = page.getByRole('button', { name: 'この期間で見る' });
  const err = page.locator('.period-controls .field-error');
  const stale = page.locator('.report-stale');
  await expect(stale).toBeHidden();
  await from.fill(`${y}-07-01`);
  await apply.click();
  await expect(err).toHaveText('期間の始めが、終わりより後になっています。');
  await expect(from).toHaveAttribute('aria-invalid', 'true');
  await expect(from).toHaveAttribute('aria-describedby', new RegExp(`(^| )${await err.getAttribute('id')}( |$)`));
  await expect(to).not.toHaveAttribute('aria-invalid', /./);
  await expect(from).toBeFocused();
  await expect(page).toHaveURL(new RegExp(`from=${y}-04-01&to=${y}-06-30`));
  await expect(stale).toHaveText(`前の期間（${y}-04-01 〜 ${y}-06-30）を表示中です。`);
  // 打ち直したら、押す前に誤りが消える。
  await from.fill(`${y}-05-01`);
  await expect(err).toBeEmpty();
  await expect(from).not.toHaveAttribute('aria-invalid', /./);
  await expect(stale).toBeHidden();
  // 終わりの欄で Enter を押しても、直す欄（始め）へ移る。
  await to.fill(`${y}-03-31`);
  await to.press('Enter');
  await expect(err).toHaveText('期間の始めが、終わりより後になっています。');
  await expect(from).toBeFocused();
  await expect(stale).toBeVisible();
  // 直して見ると、新しい期間になり、一言は消える。
  await to.fill(`${y}-06-30`);
  await apply.click();
  await expect(page).toHaveURL(new RegExp(`from=${y}-05-01&to=${y}-06-30`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${y}-05-01 〜 ${y}-06-30`);
  await expect(stale).toBeHidden();
});

// SC-06 月次推移（見た目の採点 第2回）: グラフは12月まであるのに、表が今月で終わる理由が画面に無かった。表の直前に、
// 表に出ない月と、出る条件を書く。今月より後の月を表に出しているときは「表は今月までです」を付けない（表の題と食い違うため）。
test('SC-06 月次推移の表の直前に、表に出ない月と出る条件を書く', async ({ page }) => {
  const thisMonth = todayTokyo().slice(0, 7);
  await open(page, '#/reports/monthly');
  let checked = 0;
  // 今年（今月より後の月）と、来年（記録のある月だけが表に出る）。
  for (const year of [Number(thisMonth.slice(0, 4)), Number(thisMonth.slice(0, 4)) + 1]) {
    await page.goto(`/#/reports/monthly?year=${year}`);
    await expect(page.locator('.report-title')).toHaveText(`月次推移　${year}年`);
    const report = (await apiCall(page, 'GET', `/reports/income-statement?from=${year}-01-01&to=${year}-12-31&by=month`)).json;
    const raw = [...report.revenue.rows, ...report.expense.rows];
    const shown = report.months.filter((m, i) => m.month <= thisMonth || raw.some((r) => r.months[i] !== 0));
    const hidden = report.months.filter((m) => !shown.includes(m));
    const section = page.locator('section[aria-labelledby="rt-sum-title"]');
    await expect(section.getByRole('table')).toBeVisible();
    const note = section.locator('.report-note');
    if (!hidden.length) {
      await expect(note, `${year}年`).toHaveCount(0);
      continue;
    }
    // 「表は今月までです。」は、今年で、今月より後の月を表に出していないときだけ。
    const head = year === Number(thisMonth.slice(0, 4)) && !shown.some((m) => m.month > thisMonth) ? '表は今月までです。' : '';
    const [first, last] = [hidden[0], hidden.at(-1)].map((m) => `${Number(m.month.slice(5, 7))}月`);
    const months = hidden.length === 1 ? first : `${first}.*${last}`;
    await expect(note, `${year}年`).toHaveText(new RegExp(`^${head}${months}は、その月になるか、記録ができたら出ます。$`));
    // 表の直前（見出しと表のあいだ）。
    expect(await section.evaluate((s) => [...s.children].map((c) => c.tagName.toLowerCase() + (c.classList.contains('report-note') ? '.note' : '')))).toEqual(['h3', 'p.note', 'div']);
    checked += 1;
  }
  expect(checked, '試験の前提: 表に出ない月のある年を1つ以上確かめた').toBeGreaterThan(0);
});

// SC-06 幅768（見た目の採点 第2回）: 損益計算書・貸借対照表の表が本文の約7割にとどまり、右に空白が続いた。1列に並ぶ幅では、
// 表を本文の幅いっぱいにする。
test('SC-06 幅768の損益計算書と貸借対照表は、表が本文の幅いっぱいに広がる', async ({ page }) => {
  await page.setViewportSize({ width: 768, height: 1024 });
  for (const [id, sel] of [['income-statement', '.rt-pair > .table-wrap'], ['balance-sheet', '.bs-grid > .table-wrap']]) {
    await open(page, `#/reports/${id}`);
    await expect(page.locator(sel).first()).toBeVisible();
    const body = await page.locator('.report-content .report-body').boundingBox();
    const widths = await page.locator(sel).evaluateAll((els) => els.map((e) => e.getBoundingClientRect().width));
    expect(widths.length, id).toBe(2);
    for (const w of widths) expect(w, `${id} の表の幅 ${w}px と本文の幅 ${body.width}px`).toBeGreaterThanOrEqual(body.width - 1);
  }
});

// SC-06 読み上げの名前にキーの文字を入れない（体験の評価 第2回 E4-9）。キーは aria-keyshortcuts で伝える。
test('SC-06 期間の前後のボタンは、名前にキーの文字が無く、キーは aria-keyshortcuts で伝える', async ({ page }) => {
  await open(page, '#/reports/income-statement');
  await expect(page.getByRole('button', { name: '前の年', exact: true })).toHaveAttribute('aria-keyshortcuts', '[');
  await expect(page.getByRole('button', { name: '次の年', exact: true })).toHaveAttribute('aria-keyshortcuts', ']');
  await page.goto('/#/reports/balance-sheet');
  await expect(page.getByRole('button', { name: '前の月末', exact: true })).toHaveAttribute('aria-keyshortcuts', '[');
  await expect(page.getByRole('button', { name: '次の月末', exact: true })).toHaveAttribute('aria-keyshortcuts', ']');
});
