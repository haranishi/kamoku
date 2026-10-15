// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の表の組み立て。科目の列は、親子の深さだけ字下げする。金額は右寄せの等幅の数字で、負は「-1,200」（SC-06）。
// 字下げの量は CSS の変数（--depth）で渡す（style 属性はCSPで止まるので、CSSOM で入れる）。
// 枠に入りきらない表は、横に隠れる列を作らないよう、行をカードに積み上げる（AC-11.6）。切り替えは、表の形で測って
// 決める（fitToWidth）。枠の幅の決まった線で切り替えると、大きな金額や、12か月そろった月別の表で列が隠れたため。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { money } from '../components/money.js';
import { phrase } from '../components/phrase.js';
import { focusWhenScrollable } from '../components/table.js';

/**
 * 表の中の金額（単位なし）。null なら空。
 * 0 は薄い色にする（実際の金額が先に目に入るように）。0 は意味のある数字なので「—」には置き換えない。
 */
export function yen(n) {
  if (typeof n !== 'number') return '';
  return n === 0 ? h('span', { class: 'rt-zero' }, '0') : formatYen(n);
}

/**
 * 帳票の表を作る。
 * @param {{ caption: string, head: string[], numFrom?: number, className?: string,
 *           fold?: 'cards'|'months', keyCol?: number,
 *           body: Array<{ kind?: 'group'|'row'|'total'|'grand', name: string, sub?: string, depth?: number, parent?: boolean,
 *                         direct?: boolean, cells?: Array<string|Node>, note?: Node }> }} o
 *   numFrom: この番号の列から右寄せの数字（既定は1。最初の列が科目の名前）。
 *   fold: 枠が狭いときの畳み方。'cards' は1行を1枚のカードに（見出しが科目、数字はラベルつき）、
 *         'months' は12か月の列を、カードの中の月の升目に。指定が無ければ、いつも表。
 *   keyCol: カードで太くする数字の列（cells の番号）。
 */
export function reportTable(o) {
  const numFrom = o.numFrom ?? 1;
  const width = o.head.length;
  const rows = o.body.map((r) => {
    if (r.kind === 'fill') {
      // 左右に並べた表の、合計の行の高さをそろえるための空き（中身は無いので読み上げない）。
      return h('tr', { class: 'rt-fill', 'aria-hidden': 'true' }, h('td', { colspan: String(width) }));
    }
    if (r.kind === 'group') {
      // 「負債」などの見出しは、表を横にスクロールしても左に残す（CSS の .rt-group-label）。
      return h('tr', { class: 'rt-group' }, h('th', { scope: 'colgroup', colspan: String(width) }, h('span', { class: 'rt-group-label' }, r.name)));
    }
    const nameCell = h(
      'th',
      { scope: 'row', class: ['rt-name', r.parent && 'is-parent', r.direct && 'is-direct'] },
      h('span', { class: 'rt-name-text keep' }, r.name),
      r.sub ? h('span', { class: 'rt-name-sub keep' }, r.sub) : null,
      r.note || null,
    );
    const tr = h(
      'tr',
      { class: [r.kind === 'total' && 'rt-total', r.kind === 'grand' && 'rt-grand', r.parent && 'rt-parent'] },
      nameCell,
      (r.cells || []).map((c, i) => h('td', { class: [i + 1 >= numFrom && 'num', i === o.keyCol && 'rt-key'], 'data-label': o.head[i + 1] }, c)),
    );
    // 行に置くと、カードの字下げにも、表の科目の列の字下げにも効く。
    if (r.depth) tr.style.setProperty('--depth', String(r.depth));
    return tr;
  });
  const table = h(
    'table',
    { class: ['table', 'report-table', o.fold && `rt-fold-${o.fold}`, o.className] },
    h('caption', null, o.caption),
    h('thead', null, h('tr', null, o.head.map((t, i) => h('th', { scope: 'col', class: i >= numFrom ? 'num' : null }, t)))),
    h('tbody', null, rows),
  );
  const wrap = h('div', { class: ['table-wrap', 'rt-wrap'], role: 'region', 'aria-label': o.caption }, table);
  if (o.fold) fitToWidth(wrap, table);
  focusWhenScrollable(wrap, table);
  return wrap;
}

/**
 * 表の形で枠に入るか測り、入らなければ枠に .rt-cards を付けてカードに積み上げる（screens-reports-cards.css）。
 * 見張るのは、枠の先頭に置いた高さ0の目印（枠の中の幅だけを持つ）。枠そのものを見張ると、カードと表で高さが
 * 変わるたびに知らせが来て、測り直しが続く。決めた形は data-fit（cards / table）にも出す（試験が待てるように）。
 */
function fitToWidth(wrap, table) {
  // 最初はカードの形で作り、測って入るときだけ表に戻す。閉じた開閉の中（測る知らせが来ない）でも、
  // 列を隠した表のまま残らないように。測れない環境でも、はみ出さない形（カード）のまま。
  wrap.classList.add('rt-cards');
  if (typeof ResizeObserver !== 'function') {
    wrap.dataset.fit = 'cards';
    return;
  }
  const sizer = h('div', { class: 'rt-sizer', 'aria-hidden': 'true' });
  wrap.prepend(sizer);
  let last = -1;
  const ro = new ResizeObserver((entries) => {
    if (!wrap.isConnected) {
      ro.disconnect();
      return;
    }
    const width = Math.round(entries[entries.length - 1].contentRect.width);
    // 閉じた開閉の中（幅0）では決めない。開いて幅が付いたときに決める。
    if (!width || width === last) return;
    last = width;
    wrap.classList.remove('rt-cards');
    const over = table.offsetWidth > wrap.clientWidth + 1;
    wrap.classList.toggle('rt-cards', over);
    wrap.dataset.fit = over ? 'cards' : 'table';
  });
  ro.observe(sizer);
}

/**
 * 「赤字」の札（利益が負のとき）。朱でも橙でもない無彩色の札に、注意のアイコン。
 * 赤字は、色ではなく、この文字と金額の符号で示す（AC-07.4 と同じ決まり）。
 */
export function lossNote(n) {
  return n < 0 ? h('span', { class: 'badge badge--muted rt-loss' }, icon('alert', { size: 14 }), '赤字') : null;
}

/** 一致の札（貸借の一致など）。文は塊で折る（狭い幅で語の途中から2行目にならないように）。 */
export function balanceBadge(ok, okText, ngText) {
  return ok
    ? h('span', { class: 'badge badge--ok' }, icon('check-circle', { size: 16 }), h('span', null, phrase(okText)))
    : h('span', { class: 'badge badge--danger' }, icon('alert', { size: 16 }), h('span', null, phrase(ngText)));
}

/**
 * 結論の数字（当期純利益・納付額の見込みなど）。ホームの数字のカードと同じ大きさで、表の上に出す。
 * 色だけで区別しないよう、見出しの文字と、負なら「赤字」の札を添える。
 * 見出しの補足（「（経過措置の控除を含む）」など）は hint で渡し、見出しの下の小さな2行目に出す
 * （1行に詰めると、狭いカードで「控／除」と語の途中で折れたため）。
 * @param {Array<{ label: string, hint?: string, value: number, note?: Node|null, main?: boolean }>} items
 * @param {{ className?: string }} [opts]
 */
export function reportFigures(items, opts = {}) {
  // 1億円を超える金額（11字以上）は、スマホの幅の2つ並びのカードに入らないので、そのときだけ1列に並べる（CSS）。
  const long = items.some((it) => !it.main && formatYen(it.value).length >= 11);
  return h(
    'dl',
    { class: ['rt-figures', long && 'rt-figures--long', opts.className] },
    items.map((it) =>
      h(
        'div',
        { class: ['rt-figure', it.main && 'is-main'] },
        h('dt', null, phrase(it.label), it.hint ? h('span', { class: 'rt-figure-hint' }, phrase(`（${it.hint}）`)) : null),
        h('dd', null, money(it.value), it.note || null),
      ),
    ),
  );
}

/**
 * 帳票の下の注記（アイコンと、塊で折る文）。文の配列を渡すと、1文ずつ行を分ける（長い1行は追いにくかった）。
 * @param {string|string[]} text
 */
export function reportNote(text) {
  const lines = Array.isArray(text) ? text.map((t) => h('span', { class: 'report-note-line' }, phrase(t))) : phrase(text);
  return h('p', { class: 'small muted report-note' }, icon('info', { size: 16 }), h('span', null, lines));
}
