// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の表の組み立て。科目の列は、親子の深さだけ字下げする。金額は右寄せの等幅の数字で、負は「-1,200」（SC-06）。
// 字下げの量は CSS の変数（--depth）で渡す（style 属性はCSPで止まるので、CSSOM で入れる）。

import { h } from '../dom.js';
import { formatYen } from '../format.js';

/** 金額のセル。null なら空。 */
export function money(n) {
  return typeof n === 'number' ? formatYen(n) : '';
}

/**
 * 帳票の表を作る。
 * @param {{ caption: string, head: string[], numFrom?: number, className?: string,
 *           body: Array<{ kind?: 'group'|'row'|'total'|'grand', name: string, depth?: number, parent?: boolean, direct?: boolean,
 *                         cells?: Array<string|Node>, note?: Node }> }} o
 *   numFrom: この番号の列から右寄せの数字（既定は1。最初の列が科目の名前）。
 */
export function reportTable(o) {
  const numFrom = o.numFrom ?? 1;
  const width = o.head.length;
  const rows = o.body.map((r) => {
    if (r.kind === 'group') {
      // 「負債」などの見出しは、表を横にスクロールしても左に残す（CSS の .rt-group-label）。
      return h('tr', { class: 'rt-group' }, h('th', { scope: 'colgroup', colspan: String(width) }, h('span', { class: 'rt-group-label' }, r.name)));
    }
    const nameCell = h('th', { scope: 'row', class: ['rt-name', r.parent && 'is-parent', r.direct && 'is-direct'] }, r.name, r.note || null);
    if (r.depth) nameCell.style.setProperty('--depth', String(r.depth));
    return h(
      'tr',
      { class: [r.kind === 'total' && 'rt-total', r.kind === 'grand' && 'rt-grand', r.parent && 'rt-parent'] },
      nameCell,
      (r.cells || []).map((c, i) => h('td', { class: i + 1 >= numFrom ? 'num' : null, 'data-label': o.head[i + 1] }, c)),
    );
  });
  return h(
    'div',
    { class: 'table-wrap', role: 'region', 'aria-label': o.caption, tabindex: '0' },
    h(
      'table',
      { class: ['table', 'report-table', o.className] },
      h('caption', null, o.caption),
      h('thead', null, h('tr', null, o.head.map((t, i) => h('th', { scope: 'col', class: i >= numFrom ? 'num' : null }, t)))),
      h('tbody', null, rows),
    ),
  );
}

/** 「赤字」の札（利益が負のとき。色だけでなく文字と符号で示す。AC-07.4 と同じ決まり）。 */
export function lossNote(n) {
  return n < 0 ? h('span', { class: 'badge badge--warn rt-loss' }, '赤字') : null;
}

/** 一致の札（貸借の一致など）。 */
export function balanceBadge(ok, okText, ngText) {
  return ok ? h('span', { class: 'badge badge--ok' }, okText) : h('span', { class: 'badge badge--danger' }, ngText);
}
