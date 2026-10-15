// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の表（SC-03、keyboard.md の「未確認の一覧」）。行の作り方は inbox-row.js、ここは表とキー操作。
// Enter: 行を確定する行にして次の行の科目へ（候補の一覧が開いていて印があれば、選んでから進む。それは inbox-row.js の科目の欄が受ける）。
// 上下の矢印: 同じ列の隣の行へ（候補の一覧が閉じていて、変換中でないとき）。税区分の欄では、値の切り替えにする（inbox-row-parts.js）。
// x: 無視する。Shift+Space: 印を付け外し。Esc: 行の印へ。

import { h } from '../dom.js';
import { shouldIgnoreKey } from '../ime.js';
import { modLabel } from '../keys.js';
import { createInboxRow, COLUMNS } from './inbox-row.js';
import { orderCells, PCT_NOTE, PCT_NOTE_ID } from './inbox-row-parts.js';

/** 入れ物がこの幅より狭いと、表をカードの並びにする（screens-inbox-cards.css の @container と同じ境目）。 */
export const CARDS_BELOW = 1080;
/** さらにこの幅より狭いと、カードを縦に積む並びにする（同じく @container の境目）。 */
export const STACK_BELOW = 600;

// 印の列の見出しは「確定する」。「候補」は科目の欄の候補（一覧）と同じ言葉で、2つの意味に読めたため。
// 割合は、登録の画面と同じ「事業の割合」（何の割合か分からなかった。E2 第2回）。金額は単位を添える（一覧の中に単位が無かった）。
const HEAD = { cand: '確定する', date: '日付', desc: '摘要', amount: '金額（円）', account: '科目', tax: '税区分', partner: '取引先', memo: 'メモ', pct: '事業の割合', ops: '無視' };
// Shift+Space で印を付ける欄。取引先とメモは空白を打つ欄なので外す。印の欄は、標準のSpaceの切り替えに任せる。
const SHIFT_SPACE_COLS = new Set(['date', 'account', 'tax', 'pct', 'ops']);
const colId = (col) => `inbox-col-${col}`;

/**
 * 一覧の上の「科目のある行をすべて選ぶ」（どの幅でも、この文字を見せる）。表の見出しの小さなチェックと「候補」の2文字では、
 * 25行のうち科目のある5行を選んだだけで、全部を選んだように見えた。チェック済みになるのは、科目のある行が全部選ばれたときだけ。
 */
function selectAllBox(getRows, onChange) {
  const box = h('input', { type: 'checkbox', class: 'cand-check cand-all-check' });
  const count = h('span', { class: 'cand-all-count' });
  const note = h('span', { class: 'cand-all-note', role: 'status' });
  box.addEventListener('change', () => {
    // 選べる行が無くても、チェックは止めない（止めると Tab の順が行の状態で変わる）。押したら、理由を一言出す。
    if (box.checked && !getRows().some((r) => r.ready)) {
      box.checked = false;
      note.textContent = '科目のある行がありません。';
      return;
    }
    note.textContent = '';
    // 押した結果を先に控える。行に印を付けるたびに表が描き直され、この欄の checked が「一部」に戻るため、
    // ループの中で box.checked を読むと、2行目から印を外す側に回って先頭の1行しか選ばれなかった（2026-10-15 の体験の評価）。
    const on = box.checked;
    for (const r of getRows()) if (r.ready || !on) r.setCandidate(on);
    onChange();
  });
  const el = h('div', { class: 'inbox-all' }, h('label', { class: 'cand cand-all' }, box, h('span', null, '科目のある行をすべて選ぶ'), count), note);
  return {
    el,
    paint(candidates) {
      const ready = getRows().filter((r) => r.ready);
      const marked = ready.filter((r) => candidates.has(r.id)).length;
      box.checked = ready.length > 0 && marked === ready.length;
      box.indeterminate = marked > 0 && marked < ready.length;
      // 「（3行）」と下の帯の「確定する行 2件」の関係が読み分けにくかった（見た目の採点 第2回）。この一覧の科目のある行のうち、
      // いくつに印があるかを言う（下の帯は、絞り込みの外の行も数える）。
      count.textContent = ready.length ? `（${ready.length}件のうち${marked}件を選択中）` : '（0件）';
      if (ready.length) note.textContent = '';
    },
    get input() {
      return box;
    },
  };
}

/**
 * @param {{ items: any[], settings: any, accounts: any[], partners: any[], readOnly: boolean, thisYear: string, api: any,
 *           candidates: object, onCandidate: () => void, onIgnore: (row: object) => void, onStale: (err: Error) => void,
 *           onSaveState: (kind: string) => void, onLast: () => void }} o
 *   onLast: 最後の行で Enter を押したとき（確定のボタンへ移るため）。
 * @returns {{ el: HTMLElement, rows: object[] }}
 */
export function createDraftTable(o) {
  const exempt = Boolean(o.settings && o.settings.tax_mode === 'exempt');
  const rows = [];
  const all = selectAllBox(() => rows, () => o.onCandidate());
  const onCandidate = () => {
    all.paint(o.candidates);
    if (o.readOnly) all.input.disabled = true;
    o.onCandidate();
  };
  // 見出しの文字は、行の欄の読み上げの名前（aria-labelledby の前半）にも使う。割合の説明は、見出しの title と、各行の割合の欄の
  // 見えない説明（PCT_NOTE_ID。下の p）に添える。
  const ths = COLUMNS.map((c) => h('th', { scope: 'col', id: colId(c), class: [`c-${c}`, c === 'amount' && 'num'], title: c === 'pct' ? PCT_NOTE : null }, HEAD[c]));
  const tbody = h('tbody');
  const caption = `未確認の明細（Enterで確定する行にして次の行へ、${modLabel()}+Enterで確定）`;
  const table = h('table', { class: ['table', 'inbox-table', exempt && 'is-exempt'] }, h('caption', null, caption), h('thead', null, h('tr', null, ths)), tbody);
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
      // 税区分の欄の矢印は、欄が先に受けて止める（defaultPrevented）ので、ここには来ない。
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
      onCandidate,
      onNext: next,
      onIgnore: o.onIgnore,
      onStale: o.onStale,
      onSaveState: o.onSaveState,
    });
    tbody.append(row.tr, row.msgTr);
    rows.push(row);
  }
  const paint = () => {
    all.paint(o.candidates);
    if (o.readOnly) all.input.disabled = true;
  };
  paint();
  const taxTh = ths[COLUMNS.indexOf('tax')];
  const partnerTh = ths[COLUMNS.indexOf('partner')];
  let mode = null;
  /**
   * 入れ物の幅（px）で、行の欄の並びを表・カード・縦積みの順にする。並びは境目をまたいだときだけ動かす。
   * 表のときは、税区分の列（免税なら取引先の列）の位置も測り、行の下の文を寄せる位置にする（screens-inbox.css）。
   */
  function layout(width) {
    const right = [taxTh, partnerTh].find((th) => th.offsetWidth > 0);
    if (right) table.style.setProperty('--inbox-right-x', `${right.offsetLeft}px`);
    const next = width < STACK_BELOW ? 'stack' : width < CARDS_BELOW ? 'cards' : 'table';
    if (next === mode) return;
    mode = next;
    const active = document.activeElement;
    for (const r of rows) orderCells(r.tr, mode);
    // 動かした欄にフォーカスがあると外れるので、戻す。
    if (active && active.isConnected && document.activeElement !== active) active.focus({ preventScroll: true });
  }
  const pctNote = h('p', { class: 'visually-hidden', id: PCT_NOTE_ID }, `事業の割合は、${PCT_NOTE}`);
  const el = h('div', { class: 'table-wrap inbox-wrap', role: 'region', 'aria-label': '未確認の明細', tabindex: '-1' }, all.el, table, pctNote);
  // paint: 行が減ったとき（無視した）に、「すべて選ぶ」の状態を合わせ直す。
  return { el, rows, paint, layout };
}
