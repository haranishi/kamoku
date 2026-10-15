// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 表。<table> と <caption>、<th scope> で作り、横にあふれたら表の入れ物だけがスクロールする（nfr.md）。
// 行を開ける表（onOpen）は、行にフォーカスを置き、上下の矢印で動き、Enterで開く。行のフォーカスは1つだけ（roving tabindex）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';

/**
 * @typedef {object} Column
 * @property {string} key 行のどの値か（render が無いとき、row[key] を出す）
 * @property {string} label 見出し（積み上げ表示では各セルの前に出る）
 * @property {boolean} [num] 数値の列（右寄せ・等幅の数字）
 * @property {(row: any, index: number) => (Node|string|number|null)} [render]
 * @property {string} [className] セルに付ける class
 * @property {string} [stackLabel] 積み上げ表示の見出し。'' なら出さない（行の題に使う列）
 * @property {'ascending'|'descending'|'none'} [sort] 並べ替えの状態（並べ替えできる列だけに付ける）
 * @property {() => void} [onSort] 見出しを押したときに呼ぶ
 */

/**
 * 表の入れ物を、横にスクロールできるときだけ Tab の止まり場所にする。止まれると、キーボードだけで横に動かせる
 * （表の中だけが動く）。動かない表の入れ物にまで止まると、Tab が1つ余計に要り、読み上げも領域を1つ余計に読んだ。
 * 幅や行が変わったら測り直す（入れ物と、中の表の大きさを見張る）。測れない環境では、今までどおりいつも止まる。
 * @param {HTMLElement} wrap 横にスクロールする入れ物
 * @param {HTMLElement} [inner] 中の表（行を入れ替えると、入れ物の大きさは変わらずに中の幅だけが変わるため）
 */
export function focusWhenScrollable(wrap, inner) {
  if (typeof ResizeObserver !== 'function') {
    wrap.setAttribute('tabindex', '0');
    return;
  }
  const ro = new ResizeObserver(() => {
    if (!wrap.isConnected) {
      ro.disconnect();
      return;
    }
    if (wrap.scrollWidth > wrap.clientWidth + 1) wrap.setAttribute('tabindex', '0');
    else wrap.removeAttribute('tabindex');
  });
  ro.observe(wrap);
  if (inner) ro.observe(inner);
}

/**
 * 表を作る。
 * 例: createTable({ caption: '仕訳の一覧', columns, rows, rowKey: (r) => r.id, onOpen: (r) => open(r) })
 * @param {{ caption: string, columns: Column[], rows?: any[], rowKey?: (row: any) => string,
 *           onOpen?: (row: any, index: number) => void, stack?: boolean, rowClass?: (row: any) => string|null,
 *           foot?: Node, className?: string }} opts
 */
export function createTable(opts) {
  const { caption, columns, onOpen, stack = true } = opts;
  let rows = [];
  let current = 0;

  const thead = h(
    'thead',
    null,
    h(
      'tr',
      null,
      columns.map((c) => {
        const attrs = { scope: 'col', class: [c.num && 'num', c.headClass] };
        if (c.sort) attrs['aria-sort'] = c.sort;
        const content = c.onSort
          ? h('button', { type: 'button', class: 'link-btn', onclick: c.onSort }, c.label, icon(c.sort === 'ascending' ? 'arrow-up' : 'arrow-down', { size: 14 }))
          : c.label;
        return h('th', attrs, content);
      }),
    ),
  );
  const tbody = h('tbody');
  const table = h('table', { class: ['table', stack && 'table--stack', opts.className] }, h('caption', null, caption), thead, tbody, opts.foot || null);
  const el = h('div', { class: 'table-wrap', role: 'region', 'aria-label': caption }, table);
  focusWhenScrollable(el, table);

  function cell(c, row, i) {
    const value = c.render ? c.render(row, i) : row[c.key];
    const label = c.stackLabel !== undefined ? c.stackLabel : c.label;
    return h('td', { class: [c.num && 'num', c.className], 'data-label': label }, value === undefined || value === null ? '' : value);
  }

  function rowEls() {
    return [...tbody.children].filter((tr) => tr.dataset.index !== undefined);
  }

  function setRows(next) {
    rows = next || [];
    tbody.replaceChildren();
    rows.forEach((row, i) => {
      const tr = h(
        'tr',
        { class: opts.rowClass ? opts.rowClass(row) : null, dataset: { index: i, key: opts.rowKey ? opts.rowKey(row) : i } },
        columns.map((c) => cell(c, row, i)),
      );
      if (onOpen) {
        tr.tabIndex = i === current ? 0 : -1;
        tr.addEventListener('click', (e) => {
          if (e.target.closest('a, button, input, select, textarea')) return;
          focusRow(i);
          onOpen(row, i);
        });
      }
      tbody.append(tr);
    });
    if (onOpen && rows.length && current >= rows.length) focusRow(rows.length - 1, false);
  }

  /** i 番目の行にフォーカスを移す。focus が偽なら、印（tabindex）だけ動かす。 */
  function focusRow(i, focus = true) {
    const trs = rowEls();
    if (!trs.length) return;
    const n = Math.max(0, Math.min(i, trs.length - 1));
    trs.forEach((tr, j) => {
      tr.tabIndex = j === n ? 0 : -1;
    });
    current = n;
    if (focus) {
      trs[n].focus();
      trs[n].scrollIntoView({ block: 'nearest' });
    }
  }

  if (onOpen) {
    tbody.addEventListener('keydown', (e) => {
      if (shouldIgnoreKey(e)) return;
      const tr = e.target.closest('tr[data-index]');
      if (!tr || e.target !== tr) return;
      const i = Number(tr.dataset.index);
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        focusRow(i + 1);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        focusRow(i - 1);
      } else if (e.key === 'Home') {
        e.preventDefault();
        focusRow(0);
      } else if (e.key === 'End') {
        e.preventDefault();
        focusRow(rows.length - 1);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        onOpen(rows[i], i);
      }
    });
    tbody.addEventListener('focusin', (e) => {
      const tr = e.target.closest('tr[data-index]');
      if (tr && e.target === tr) focusRow(Number(tr.dataset.index), false);
    });
  }

  if (opts.rows) setRows(opts.rows);

  return {
    el,
    table,
    tbody,
    setRows,
    focusRow,
    /** いま印のある行の番号。 */
    get currentIndex() {
      return current;
    },
    /** 行にフォーカスがあれば、その番号。無ければ -1。 */
    focusedIndex() {
      const tr = document.activeElement && document.activeElement.closest ? document.activeElement.closest('tr[data-index]') : null;
      return tr && tbody.contains(tr) ? Number(tr.dataset.index) : -1;
    },
    rowAt(i) {
      return rows[i];
    },
    get length() {
      return rows.length;
    },
  };
}
