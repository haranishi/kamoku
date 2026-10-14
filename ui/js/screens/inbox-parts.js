// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の画面の部品: 「無視した行」の表（元に戻す）と、空の状態（AC-05.7、SC-03）。

import { h } from '../dom.js';
import { formatSigned } from '../format.js';
import { createTable } from '../components/table.js';
import { emptyState } from '../components/empty.js';
import { button } from '../components/fields.js';
import { shortDate } from './inbox-row.js';

/**
 * 取り込まないことにした行の一覧。行ごとに「戻す」。
 * @param {{ items: any[], thisYear: string, readOnly: boolean, onRestore: (draft: object) => void }} p
 */
export function ignoredTable({ items, thisYear, readOnly, onRestore }) {
  const t = createTable({
    caption: '取り込まない行（「戻す」で未確認に戻る）',
    className: 'ignored-table',
    rowKey: (d) => d.id,
    columns: [
      { key: 'date', label: '日付', render: (d) => shortDate(d.date, thisYear) },
      { key: 'description', label: '摘要', className: 'wrap', stackLabel: '', render: (d) => [h('span', null, d.description || '（摘要なし）'), h('span', { class: 'sub-inline' }, d.wallet_name)] },
      { key: 'amount', label: '金額', num: true, render: (d) => h('span', { class: ['amount', `amount--${d.direction}`] }, formatSigned(d.direction === 'in' ? d.amount : -d.amount)) },
      {
        key: 'ops',
        label: '操作',
        stackLabel: '',
        render: (d) => {
          const b = button({ label: '戻す', iconName: 'refresh', small: true, 'aria-label': `未確認に戻す: ${d.description}`, onClick: () => onRestore(d) });
          b.disabled = readOnly;
          return b;
        },
      },
    ],
    rows: items,
  });
  return t.el;
}

/**
 * 空の状態。未確認が0件なら、取込と登録の2つの入口（screens-main.md）。
 * @param {{ status: 'draft'|'ignored', filtered: boolean, onClear: () => void }} p
 */
export function inboxEmpty({ status, filtered, onClear }) {
  if (filtered) {
    return h('div', { class: 'card' }, emptyState({ iconName: 'search', title: '条件に合う明細はありません', body: '口座や探す言葉を変えるか、「条件を消す」で全部の明細に戻してください。', actions: [{ label: '条件を消す', onClick: onClear }] }));
  }
  if (status === 'ignored') {
    return h('div', { class: 'card' }, emptyState({ iconName: 'ban', title: '取り込まない行はありません', body: '未確認の表で x を押した行が、ここに並びます。「戻す」で未確認に戻せます。' }));
  }
  return h(
    'div',
    { class: 'card' },
    emptyState({
      iconName: 'inbox',
      title: '未確認はありません',
      body: '明細を取り込むか、取引を登録してください。',
      actions: [
        { label: '明細を取り込む', href: '#/import', primary: true },
        { label: '取引を登録', href: '#/entry', kbd: 'n' },
      ],
    }),
  );
}
