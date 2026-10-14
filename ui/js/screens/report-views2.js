// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の中身（2）: 試算表・総勘定元帳・消費税（SC-06、api-reports.md）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { treeRows } from '../report-tree.js';
import { createTable } from '../components/table.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { reportTable, money, balanceBadge } from './report-table.js';

const KINDS = [
  ['asset', '資産'],
  ['liability', '負債'],
  ['equity', '資本'],
  ['revenue', '収益'],
  ['expense', '費用'],
];

/** 試算表: 期首・借方・貸方・期末の4つの数字の列。行は科目ごと（親の行は子の合計）。 */
export async function trialView({ api, period, accounts }) {
  const res = await api.get('/reports/trial-balance', { from: period.from, to: period.to });
  const keys = ['opening', 'debit', 'credit', 'closing'];
  const body = [];
  for (const [kind, label] of KINDS) {
    const rows = res.rows.filter((r) => r.kind === kind);
    if (!rows.length) continue;
    body.push({ kind: 'group', name: label });
    for (const r of treeRows(rows, accounts, keys)) body.push({ name: r.name, depth: r.depth, parent: r.parent, direct: r.direct, cells: keys.map((k) => money(r[k])) });
  }
  body.push({ kind: 'grand', name: '動きの合計', cells: ['', money(res.total_debit), money(res.total_credit), ''] });
  const node = h(
    'div',
    { class: 'report-body' },
    h('p', { class: 'report-check' }, balanceBadge(res.balanced, '借方と貸方の合計、期末の残高の貸借が一致しています', '貸借が一致していません。設定の「データ」で台帳を検査してください')),
    res.rows.length
      ? reportTable({ caption: '試算表', head: ['科目', '期首', '借方', '貸方', '期末'], body })
      : h('div', { class: 'card' }, emptyState({ iconName: 'book', title: 'この期間には、残高も動きもありません', body: '期間を変えるか、取引を登録してください。' })),
    h('p', { class: 'small muted report-note' }, icon('info', { size: 16 }), h('span', null, '期首と期末は、科目のふだんの側から見た残高です（資産と費用は借方−貸方、負債・資本・収益は貸方−借方）。前の年までの利益は、元入金の期首に入れています。')),
  );
  return { node, csv: { path: '/export/trial-balance.csv', params: { from: period.from, to: period.to } } };
}

/**
 * 総勘定元帳: 科目を選ぶと、日付順の行と残高の推移。行から仕訳の詳細に飛べる（Enter、またはNoのリンク）。
 * @param {{ api: any, period: object, account: string|null, navigate: (hash: string) => void }} o
 */
export async function ledgerView({ api, period, account, navigate }) {
  if (!account) {
    return { node: h('div', { class: 'card' }, emptyState({ iconName: 'list', title: '科目を選んでください', body: '上の「科目」の欄で選ぶと、その科目の動きと残高の推移が出ます。口座を選ぶと、口座の出入りが見られます。' })), csv: null };
  }
  const res = await api.get('/reports/ledger', { account, from: period.from, to: period.to });
  const table = createTable({
    caption: '総勘定元帳（行で Enter を押すと、その仕訳を開く）',
    className: 'ledger-table',
    rowKey: (r) => `${r.entry_id}-${r.no}-${r.balance}`,
    onOpen: (r) => navigate(`#/journal/${encodeURIComponent(r.entry_id)}`),
    columns: [
      { key: 'date', label: '日付', render: (r) => r.date },
      { key: 'no', label: 'No', num: true, render: (r) => h('a', { href: `#/journal/${encodeURIComponent(r.entry_id)}`, tabindex: '-1' }, `No.${r.no}`) },
      { key: 'description', label: '摘要', className: 'wrap', render: (r) => r.description || '（摘要なし）' },
      { key: 'counter', label: '相手科目', render: (r) => r.counter },
      { key: 'debit', label: '借方', num: true, render: (r) => (r.debit ? formatYen(r.debit) : '') },
      { key: 'credit', label: '貸方', num: true, render: (r) => (r.credit ? formatYen(r.credit) : '') },
      { key: 'balance', label: '残高', num: true, render: (r) => formatYen(r.balance) },
    ],
    rows: res.rows,
  });
  const node = h(
    'div',
    { class: 'report-body' },
    h(
      'dl',
      { class: 'ledger-facts' },
      h('div', null, h('dt', null, '期首の残高'), h('dd', null, h('span', { class: 'num' }, money(res.opening)), '円')),
      h('div', null, h('dt', null, '期間の行'), h('dd', null, h('span', { class: 'num' }, String(res.rows.length)), '行')),
      h('div', null, h('dt', null, '期末の残高'), h('dd', null, h('span', { class: 'num' }, money(res.closing)), '円')),
    ),
    res.rows.length ? table.el : h('div', { class: 'card' }, emptyState({ iconName: 'book', title: 'この期間には、この科目の動きがありません', body: '期間を変えるか、別の科目を選んでください。' })),
  );
  return { node, csv: { path: '/export/ledger.csv', params: { account, from: period.from, to: period.to } } };
}

/** 消費税: 税区分ごとの課税標準と税額、経過措置の内訳、納付額の見込み（参考）。上に参考値の注意を常に出す。 */
export async function taxView({ api, period, settings }) {
  const res = await api.get('/reports/tax-summary', { from: period.from, to: period.to });
  const notices = [];
  if (settings && settings.tax_mode === 'exempt') notices.push(banner({ kind: 'info', title: '設定上は免税事業者として表示しています。申告の要否は、国税庁の案内か税理士に確かめてください', body: '課税事業者になったときは、設定の「事業者」で課税の区分を変えてください。' }));
  notices.push(banner({ kind: 'warn', title: '参考値。簡易課税・特例は反映していません', body: res.disclaimer }));
  const byCode = reportTable({
    caption: '税区分ごとの課税標準と税額',
    head: ['税区分', '課税標準（税込）', '含まれる税額'],
    body: res.rows.length ? res.rows.map((r) => ({ name: r.label, cells: [money(r.gross), money(r.tax)] })) : [{ name: 'この期間には、税区分のある取引がありません', cells: ['', ''] }],
  });
  const summary = h(
    'dl',
    { class: 'tax-sums' },
    h('div', null, h('dt', null, '売上税額'), h('dd', null, h('span', { class: 'num' }, money(res.sales_tax)), '円')),
    h('div', null, h('dt', null, '仕入税額（経過措置の控除を含む）'), h('dd', null, h('span', { class: 'num' }, money(res.purchase_tax)), '円')),
    h('div', { class: 'tax-estimate' }, h('dt', null, '納付額の見込み（参考・本則課税）'), h('dd', null, h('span', { class: 'num' }, money(res.estimate)), '円')),
  );
  const trans = res.transitional.length
    ? reportTable({
        caption: '経過措置の内訳（免税事業者等からの課税仕入れ）',
        head: ['控除の割合', '支払額（税込）', '含まれる税額', '控除できる額'],
        body: res.transitional.map((t) => ({ name: `${t.percent}%`, cells: [money(t.gross), money(t.tax), money(t.deductible)] })),
      })
    : null;
  const node = h(
    'div',
    { class: 'report-body report-narrow' },
    notices,
    byCode,
    summary,
    trans ? h('section', { class: 'report-section', 'aria-labelledby': 'tax-trans-title' }, h('h3', { id: 'tax-trans-title' }, '経過措置の内訳'), trans) : null,
  );
  return { node, csv: { path: '/export/tax-summary.csv', params: { from: period.from, to: period.to } } };
}
