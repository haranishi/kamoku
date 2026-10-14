// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の表（SC-03、keyboard.md の「未確認の一覧」）。行の作り方は inbox-row.js、ここは表とキー操作。
// Enter: 行を確定候補にして次の行の科目へ（候補の一覧が開いていて印があれば、選んでから進む。それは inbox-row.js の科目の欄が受ける）。
// 上下の矢印: 同じ列の隣の行へ（候補の一覧が閉じていて、変換中でないとき）。x: 取り込まない。Shift+Space: 印を付け外し。Esc: 行の印へ。

import { h } from '../dom.js';
import { shouldIgnoreKey } from '../ime.js';
import { createInboxRow, COLUMNS } from './inbox-row.js';

const HEAD = { cand: '候補', date: '日付', desc: '摘要', amount: '金額', account: '科目', tax: '税区分', partner: '取引先', memo: 'メモ', pct: '割合', ops: '操作' };
// Shift+Space で印を付ける欄。取引先とメモは空白を打つ欄なので外す。候補の欄は、標準のSpaceの切り替えに任せる。
const SHIFT_SPACE_COLS = new Set(['date', 'account', 'tax', 'pct', 'ops']);
const colId = (col) => `inbox-col-${col}`;

/**
 * @param {{ items: any[], settings: any, accounts: any[], partners: any[], readOnly: boolean, thisYear: string, api: any,
 *           candidates: object, onCandidate: () => void, onIgnore: (row: object) => void, onStale: (err: Error) => void,
 *           onSaveState: (kind: string) => void, onLast: () => void }} o
 *   onLast: 最後の行で Enter を押したとき（確定のボタンへ移るため）。
 * @returns {{ el: HTMLElement, rows: object[] }}
 */
export function createDraftTable(o) {
  const exempt = Boolean(o.settings && o.settings.tax_mode === 'exempt');
  const ths = COLUMNS.map((c) => h('th', { scope: 'col', id: colId(c), class: [`c-${c}`, c === 'amount' && 'num'] }, c === 'ops' ? h('span', { class: 'visually-hidden' }, HEAD[c]) : HEAD[c]));
  const tbody = h('tbody');
  const table = h('table', { class: ['table', 'inbox-table', exempt && 'is-exempt'] }, h('caption', null, '未確認の明細（Enterで確定候補にして次の行へ、Ctrl+Enterで確定）'), h('thead', null, h('tr', null, ths)), tbody);
  const rows = [];
  const rowOf = (el) => rows.find((r) => r.tr === el.closest('tr.inbox-row'));

  function next(row) {
    const target = rows[rows.indexOf(row) + 1];
    if (target) target.focus('account');
    else o.onLast();
  }

  tbody.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.defaultPrevented || e.ctrlKey || e.metaKey || e.altKey) return;
    const row = rowOf(e.target);
    if (!row) return;
    const td = e.target.closest('td');
    const col = td ? td.dataset.col : null;
    if (e.key === 'Enter' && !e.shiftKey) {
      if (e.target.tagName === 'BUTTON') return;
      e.preventDefault();
      row.markAndNext(col);
    } else if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && !e.shiftKey && !row.isOpen()) {
      e.preventDefault();
      const target = rows[rows.indexOf(row) + (e.key === 'ArrowDown' ? 1 : -1)];
      if (target) target.focus(col);
    } else if ((e.key === 'x' || e.key === 'X') && (col === 'cand' || col === 'ops')) {
      e.preventDefault();
      o.onIgnore(row);
    } else if (e.key === ' ' && e.shiftKey && SHIFT_SPACE_COLS.has(col)) {
      e.preventDefault();
      row.setCandidate(!o.candidates.has(row.id));
    } else if (e.key === 'Escape' && col !== 'cand' && !row.isOpen()) {
      e.preventDefault();
      row.focus('cand');
    }
  });

  for (const draft of o.items) {
    const row = createInboxRow({
      draft,
      accounts: o.accounts,
      partners: o.partners,
      exempt,
      readOnly: o.readOnly,
      thisYear: o.thisYear,
      colId,
      api: o.api,
      candidates: o.candidates,
      onCandidate: o.onCandidate,
      onNext: next,
      onIgnore: o.onIgnore,
      onStale: o.onStale,
      onSaveState: o.onSaveState,
    });
    tbody.append(row.tr, row.msgTr);
    rows.push(row);
  }
  const el = h('div', { class: 'table-wrap inbox-wrap', role: 'region', 'aria-label': '未確認の明細', tabindex: '-1' }, table);
  return { el, rows };
}
