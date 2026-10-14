// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の中身（1）: 月次推移・損益計算書・貸借対照表（SC-06、api-reports.md）。
// 各関数は { node, csv: { path, params }, destroy? } を返す。区分の合計はサーバーの合計を使う（画面で足し直さない）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { treeRows, flattenBsRows } from '../report-tree.js';
import { createChart } from '../components/chart.js';
import { reportTable, money, lossNote, balanceBadge } from './report-table.js';

const MONTHS = Array.from({ length: 12 }, (_, i) => `${i + 1}月`);

/** 月次推移: 棒（売上と経費）と線（利益）のグラフに、同じ数字の表を並べる。科目ごとの月別も出す。 */
export async function monthlyView({ api, period, accounts, today }) {
  const res = await api.get('/reports/income-statement', { from: period.from, to: period.to, by: 'month' });
  const series = res.months.map((m) => ({ month: m.month, revenue: m.revenue, expense: m.expense, profit: m.net_income }));
  const thisYear = Number(today.slice(0, 4));
  // 今月より後の月は描かない（0へ落ちたように見えるため。screens-main.md のホームと同じ決まり）。
  const until = period.year < thisYear ? `${period.year}-12` : period.year === thisYear ? today.slice(0, 7) : `${period.year}-00`;
  const chart = createChart({ series, until, label: `${period.year}年の月次推移` });
  const shown = series.filter((r) => r.month <= until);
  const sum = (k) => shown.reduce((a, r) => a + r[k], 0);
  const summary = reportTable({
    caption: `${period.year}年の月次推移（グラフと同じ数字）`,
    head: ['月', '売上', '経費', '利益'],
    className: 'rt-months',
    body: [
      ...shown.map((r) => ({ name: `${Number(r.month.slice(5, 7))}月`, cells: [money(r.revenue), money(r.expense), money(r.profit)], note: lossNote(r.profit) })),
      { kind: 'grand', name: '合計', cells: [money(sum('revenue')), money(sum('expense')), money(sum('profit'))], note: lossNote(sum('profit')) },
    ],
  });
  const section = (title, sec) => [
    { kind: 'group', name: title },
    ...treeRows(sec.rows, accounts, ['amount', 'months']).map((r) => ({ name: r.name, depth: r.depth, parent: r.parent, direct: r.direct, cells: [...r.months.map(money), money(r.amount)] })),
    { kind: 'total', name: `${title}の合計`, cells: [...MONTHS.map((_, i) => money(sec.rows.reduce((a, r) => a + r.months[i], 0))), money(sec.total)] },
  ];
  const byAccount = reportTable({
    caption: `${period.year}年の科目ごとの月別`,
    head: ['科目', ...MONTHS, '合計'],
    className: 'rt-wide',
    body: [...section('収益', res.revenue), ...section('費用', res.expense), { kind: 'grand', name: '当期純利益', cells: [...res.months.map((m) => money(m.net_income)), money(res.net_income)], note: lossNote(res.net_income) }],
  });
  const node = h(
    'div',
    { class: 'report-body' },
    h('section', { class: 'card chart-card', 'aria-label': 'グラフ' }, chart.el),
    h('section', { class: 'report-section report-narrow', 'aria-labelledby': 'rt-sum-title' }, h('h3', { id: 'rt-sum-title' }, '月ごとの数字'), summary),
    h('details', { class: 'report-section report-more' }, h('summary', null, '科目ごとの月別を見る'), byAccount),
  );
  return { node, csv: { path: '/export/income-statement.csv', params: { by: 'month', from: period.from, to: period.to } }, destroy: () => chart.destroy() };
}

/** 損益計算書: 収益と費用（親の下に子をインデント）、当期純利益。負の利益は「赤字」と符号で示す。 */
export async function incomeView({ api, period, accounts }) {
  const res = await api.get('/reports/income-statement', { from: period.from, to: period.to, by: 'total' });
  const section = (title, sec) => [
    { kind: 'group', name: title },
    ...(sec.rows.length ? treeRows(sec.rows, accounts, ['amount']).map((r) => ({ name: r.name, depth: r.depth, parent: r.parent, direct: r.direct, cells: [money(r.amount)] })) : [{ name: 'この期間は、ありません', cells: [''] }]),
    { kind: 'total', name: `${title}の合計`, cells: [money(sec.total)] },
  ];
  const node = h(
    'div',
    { class: 'report-body report-narrow' },
    reportTable({
      caption: '損益計算書',
      head: ['科目', '金額'],
      body: [...section('収益', res.revenue), ...section('費用', res.expense), { kind: 'grand', name: '当期純利益（収益−費用）', cells: [money(res.net_income)], note: lossNote(res.net_income) }],
    }),
    h('p', { class: 'small muted report-note' }, icon('info', { size: 16 }), h('span', null, '事業主貸と事業主借は、売上にも経費にも入りません。取消済みの仕訳は集計に入りません。')),
  );
  return { node, csv: { path: '/export/income-statement.csv', params: { by: 'total', from: period.from, to: period.to } } };
}

/** 貸借対照表: 資産と、負債・資本（最後の行が当期純利益）。資産の合計＝負債と資本の合計を確かめる。 */
export async function balanceView({ api, period }) {
  const res = await api.get('/reports/balance-sheet', { as_of: period.asOf });
  const rowsOf = (sec) => (sec.rows.length ? flattenBsRows(sec.rows).map((r) => ({ name: r.name, depth: r.depth, parent: r.parent, direct: r.direct, cells: [money(r.amount)] })) : [{ name: '残高のある科目はありません', cells: [''] }]);
  const left = reportTable({ caption: '資産', head: ['資産', '金額'], body: [...rowsOf(res.assets), { kind: 'grand', name: '資産の合計', cells: [money(res.assets.total)] }] });
  const right = reportTable({
    caption: '負債と資本',
    head: ['負債と資本', '金額'],
    body: [
      { kind: 'group', name: '負債' },
      ...rowsOf(res.liabilities),
      { kind: 'total', name: '負債の合計', cells: [money(res.liabilities.total)] },
      { kind: 'group', name: '資本' },
      ...rowsOf(res.equity).map((r, i, all) => (i === all.length - 1 ? { ...r, note: lossNote(res.net_income_current) } : r)),
      { kind: 'total', name: '資本の合計', cells: [money(res.equity.total)] },
      { kind: 'grand', name: '負債と資本の合計', cells: [money(res.liabilities.total + res.equity.total)] },
    ],
  });
  const diff = res.assets.total - (res.liabilities.total + res.equity.total);
  const node = h(
    'div',
    { class: 'report-body' },
    h('p', { class: 'report-check' }, balanceBadge(res.balanced, '資産の合計と、負債と資本の合計が一致しています', `一致していません（差 ${formatYen(Math.abs(diff))}円）`)),
    h('div', { class: 'bs-grid' }, left, right),
    h('p', { class: 'small muted report-note' }, icon('info', { size: 16 }), h('span', null, '事業主貸と事業主借は、その年の動きだけです。前の年までの利益と事業主貸・事業主借は、元入金に入れて見せています（締めの仕訳は作りません）。')),
  );
  return { node, csv: { path: '/export/balance-sheet.csv', params: { as_of: period.asOf } } };
}
