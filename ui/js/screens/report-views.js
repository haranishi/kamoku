// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の中身（1）: 月次推移・損益計算書・貸借対照表（SC-06、api-reports.md）。
// 各関数は { node, csv: { path, params }, destroy? } を返す。区分の合計はサーバーの合計を使う（画面で足し直さない）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { treeRows, flattenBsRows, collapseSingleChildren, collapseSingleRows, monthColumns, hiddenMonthsNote } from '../report-tree.js';
import { createChart, isEmptyMonth } from '../components/chart.js';
import { emptyValue } from '../components/money.js';
import { reportTable, yen, lossNote, balanceBadge, reportFigures, reportNote } from './report-table.js';

const monthName = (ym) => `${Number(ym.slice(5, 7))}月`;

/**
 * 月次推移: グラフと表で今月までの数字を見比べられるようにし、金額のある後の月は表に残す。
 * 表はグラフのすぐ下に開いたまま置くので、グラフの「表で見る」は出さない。記録のない月（収益も経費も0）は「—」。
 * 表の月は、今月までと、今月より後でも科目の金額がある月（2つの表で同じ月。合計がサーバーの年間の値と CSV に合うように）。
 */
export async function monthlyView({ api, period, accounts, today }) {
  const res = await api.get('/reports/income-statement', { from: period.from, to: period.to, by: 'month' });
  const series = res.months.map((m) => ({ month: m.month, revenue: m.revenue, expense: m.expense, profit: m.net_income }));
  const thisYear = Number(today.slice(0, 4));
  // 今月より後の月は描かない（0へ落ちたように見えるため。screens-main.md のホームと同じ決まり）。
  const until = period.year < thisYear ? `${period.year}-12` : period.year === thisYear ? today.slice(0, 7) : `${period.year}-00`;
  const chart = createChart({ series, until, label: `${period.year}年の月次推移`, tableToggle: false });
  const cols = monthColumns(series, [...res.revenue.rows, ...res.expense.rows], until);
  const shown = cols.map((c) => series[c.i]);
  const sum = (k) => shown.reduce((a, r) => a + r[k], 0);
  const cell = (r, k) => (isEmptyMonth(r) ? emptyValue() : yen(r[k]));
  const future = shown.some((r) => r.month > today.slice(0, 7));
  const summary = reportTable({
    caption: `${period.year}年の月次推移（${future ? '今月より後の月は、表だけに出ます' : 'グラフと同じ数字'}）`,
    head: ['月', '売上', '経費', '利益'],
    className: 'rt-months',
    fold: 'cards',
    keyCol: 2,
    body: [
      ...shown.map((r) => ({ name: monthName(r.month), cells: [cell(r, 'revenue'), cell(r, 'expense'), cell(r, 'profit')], note: isEmptyMonth(r) ? null : lossNote(r.profit) })),
      { kind: 'grand', name: '合計', cells: [yen(sum('revenue')), yen(sum('expense')), yen(sum('profit'))], note: lossNote(sum('profit')) },
    ],
  });
  // 科目ごとの月別で「—」にするのは、どの科目にも金額が無い月だけ。振替だけの月（収益と費用の合計が0）でも、
  // 科目の金額は出す（隠すと、実際の金額が表から消える）。
  const monthCells = (values) => cols.map((c) => (c.hasAmounts ? yen(values[c.i]) : emptyValue()));
  const section = (title, sec) => [
    { kind: 'group', name: title },
    ...collapseSingleRows(treeRows(sec.rows, accounts, ['amount', 'months'])).map((r) => ({ name: r.name, sub: r.sub, depth: r.depth, parent: r.parent, direct: r.direct, cells: [...monthCells(r.months), yen(r.amount)] })),
    { kind: 'total', name: `${title}の合計`, cells: [...monthCells(series.map((_, i) => sec.rows.reduce((a, r) => a + r.months[i], 0))), yen(sec.total)] },
  ];
  const byAccount = reportTable({
    caption: `${period.year}年の科目ごとの月別`,
    head: ['科目', ...cols.map((c) => monthName(c.month)), '合計'],
    className: 'rt-wide',
    fold: 'months',
    keyCol: cols.length,
    body: [
      ...section('収益', res.revenue),
      ...section('費用', res.expense),
      { kind: 'grand', name: '当期純利益', cells: [...monthCells(res.months.map((m) => m.net_income)), yen(res.net_income)], note: lossNote(res.net_income) },
    ],
  });
  // グラフは12月まであるのに、表が今月で終わる理由が画面に無かった。表に出ない月と、出る条件を表の直前に書く。
  const hiddenText = hiddenMonthsNote(series, cols, today.slice(0, 7));
  const monthsNote = hiddenText ? reportNote(hiddenText) : null;
  const node = h(
    'div',
    { class: 'report-body' },
    h('section', { class: 'card chart-card', 'aria-label': 'グラフ' }, chart.el),
    h('section', { class: 'report-section', 'aria-labelledby': 'rt-sum-title' }, h('h3', { id: 'rt-sum-title' }, '月ごとの数字'), monthsNote, summary),
    h('details', { class: 'report-section report-more' }, h('summary', null, icon('chevron-right', { size: 18, className: 'report-more-icon' }), '科目ごとの月別を見る'), byAccount),
  );
  return { node, csv: { path: '/export/income-statement.csv', params: { by: 'month', from: period.from, to: period.to } }, destroy: () => chart.destroy() };
}

/**
 * 損益計算書: 結論（当期純利益）を表の上に大きく出し、収益と費用は、広い枠では左右に並べる（貸借対照表と同じ組み方）。
 * 親の下に子を字下げする。負の利益は「赤字」の札と符号で示す。
 */
export async function incomeView({ api, period, accounts }) {
  const res = await api.get('/reports/income-statement', { from: period.from, to: period.to, by: 'total' });
  const side = (title, sec) =>
    reportTable({
      caption: title,
      head: [title, '金額'],
      body: [
        ...(sec.rows.length
          ? collapseSingleRows(treeRows(sec.rows, accounts, ['amount'])).map((r) => ({ name: r.name, sub: r.sub, depth: r.depth, parent: r.parent, direct: r.direct, cells: [yen(r.amount)] }))
          : [{ name: 'この期間は、ありません', cells: [''] }]),
        { kind: 'grand', name: `${title}の合計`, cells: [yen(sec.total)] },
      ],
    });
  const node = h(
    'div',
    { class: 'report-body' },
    reportFigures([
      { label: '当期純利益', hint: '収益−費用', value: res.net_income, note: lossNote(res.net_income), main: true },
      { label: '収益の合計', value: res.revenue.total },
      { label: '費用の合計', value: res.expense.total },
    ]),
    h('div', { class: 'rt-pair' }, side('収益', res.revenue), side('費用', res.expense)),
    reportNote('事業主貸と事業主借は、売上にも経費にも入りません。取消済みの仕訳は集計に入りません。'),
  );
  return { node, csv: { path: '/export/income-statement.csv', params: { by: 'total', from: period.from, to: period.to } } };
}

/**
 * 貸借対照表: 資産と、負債・資本（最後の行が当期純利益）。資産の合計＝負債と資本の合計を確かめる。
 * 左右の合計の行は、同じ高さにそろえる（短い側の表の、合計の手前を空ける。CSS の .bs-grid）。
 * 子が1つだけの親は1行に畳む（同じ金額が2回並ばないように）。
 */
export async function balanceView({ api, period }) {
  const res = await api.get('/reports/balance-sheet', { as_of: period.asOf });
  const rowsOf = (sec) =>
    sec.rows.length
      ? flattenBsRows(collapseSingleChildren(sec.rows)).map((r) => ({ name: r.name, sub: r.sub, depth: r.depth, parent: r.parent, direct: r.direct, cells: [yen(r.amount)] }))
      : [{ name: '残高のある科目はありません', cells: [''] }];
  const fill = { kind: 'fill' };
  const left = reportTable({ caption: '資産', head: ['資産', '金額'], className: 'bs-table', body: [...rowsOf(res.assets), fill, { kind: 'grand', name: '資産の合計', cells: [yen(res.assets.total)] }] });
  const right = reportTable({
    caption: '負債と資本',
    head: ['負債と資本', '金額'],
    className: 'bs-table',
    body: [
      { kind: 'group', name: '負債' },
      ...rowsOf(res.liabilities),
      { kind: 'total', name: '負債の合計', cells: [yen(res.liabilities.total)] },
      { kind: 'group', name: '資本' },
      ...rowsOf(res.equity).map((r, i, all) => (i === all.length - 1 ? { ...r, note: lossNote(res.net_income_current) } : r)),
      { kind: 'total', name: '資本の合計', cells: [yen(res.equity.total)] },
      fill,
      { kind: 'grand', name: '負債と資本の合計', cells: [yen(res.liabilities.total + res.equity.total)] },
    ],
  });
  const diff = res.assets.total - (res.liabilities.total + res.equity.total);
  const node = h(
    'div',
    { class: 'report-body' },
    h('p', { class: 'report-check' }, balanceBadge(res.balanced, '資産の合計と、負債と資本の合計が一致しています', `一致していません（差 ${formatYen(Math.abs(diff))}円）`)),
    h('div', { class: 'bs-grid' }, left, right),
    reportNote('事業主貸と事業主借は、その年の動きだけです。前の年までの利益と事業主貸・事業主借は、元入金に入れて見せています（締めの仕訳は作りません）。'),
  );
  return { node, csv: { path: '/export/balance-sheet.csv', params: { as_of: period.asOf } } };
}
