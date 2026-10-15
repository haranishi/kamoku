// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の(2)の見本（SC-04）。広い幅は先頭20行の表、狭い幅（640px未満）は列ごとの縦の一覧にする。
// 狭い幅で表を横に送らせると、3列目より右（金額・残高）が枠の外に隠れたため（AC-11.6）。
// どちらも、選んだ列に役目（日付・摘要・出金など）を文字で付ける（背景の色だけに頼らない）。
// 表の行の番号は、ファイルの行番号（確かめの画面の「ファイルの○行目」と同じ数え方）。読み飛ばす行の数を推測より増やすと、
// 見本の先頭の行も読み飛ばすので、その行に「読み飛ばす」と付ける（import-map.js の sampleLines・sampleSkipped）。

import { h } from '../dom.js';
import { columnExamples, isNumberColumn, sampleLines, sampleSkipped } from '../import-map.js';
import { phrase } from '../components/phrase.js';

/**
 * 列の役目の名前（「日付」「出金」など）。選んだ対応から決める。
 * @param {object} cur いまの対応（import-columns.js の current()）
 */
export function rolesOf(cur, i) {
  const tags = [];
  if (cur.date_col === i) tags.push('日付');
  if (cur.desc_cols.includes(i)) tags.push('摘要');
  if (cur.amount.mode === 'split' && cur.amount.out_col === i) tags.push('出金');
  if (cur.amount.mode === 'split' && cur.amount.in_col === i) tags.push('入金');
  if (cur.amount.mode === 'signed' && cur.amount.col === i) tags.push('金額');
  if (cur.id_col === i) tags.push('取引番号');
  return tags;
}

/** 広い幅の見本の表。数だけの列は右に寄せる。値はファイルの中身のまま（桁区切りも足さない）。 */
function sampleTable(up, opts, cur) {
  const num = (i) => isNumberColumn(up, i);
  const lines = sampleLines(up);
  const head = h(
    'tr',
    null,
    h('th', { scope: 'col', class: 'num line-col' }, 'ファイルの行'),
    opts.map((x) => {
      const roles = rolesOf(cur, x.value);
      return h('th', { scope: 'col', class: [roles.length && 'is-mapped', num(x.value) && 'num'] }, h('span', { class: 'col-name' }, x.name || `${x.value + 1}列目`), roles.map((t) => h('span', { class: 'badge badge--ok col-tag' }, t)));
    }),
  );
  const lineCell = (i) => {
    const skipped = sampleSkipped(up, cur.skip_rows, i);
    return h('td', { class: 'num muted line-col' }, String(lines[i]), skipped ? h('span', { class: 'badge badge--muted skip-tag' }, '読み飛ばす') : null);
  };
  const body = (up.sample || []).slice(0, 20).map((r, i) =>
    h('tr', { class: sampleSkipped(up, cur.skip_rows, i) && 'is-skipped' }, lineCell(i), opts.map((x) => h('td', { class: [rolesOf(cur, x.value).length && 'is-mapped', num(x.value) && 'num'] }, String(r[x.value] ?? '')))),
  );
  return h(
    'div',
    { class: 'table-wrap sample-wrap', role: 'region', 'aria-label': 'ファイルの先頭の行（見本）', tabindex: '0' },
    h('table', { class: 'table sample-table' }, h('caption', null, 'ファイルの先頭の行（見本）。選んだ列には、役目の札が付きます'), h('thead', null, head), h('tbody', null, body)),
  );
}

/** 狭い幅の、列ごとの縦の一覧。「3列目 お支払金額 → 出金。例: 1,650」を1つのかたまりにする。 */
function columnList(up, opts, cur) {
  return h(
    'ol',
    { class: 'col-list', 'aria-label': 'ファイルの列（見本）' },
    opts.map((x) => {
      const roles = rolesOf(cur, x.value);
      const examples = columnExamples(up, x.value);
      return h(
        'li',
        { class: ['col-item', roles.length && 'is-mapped'] },
        h('p', { class: 'col-item-head' }, h('span', { class: 'col-no' }, `${x.value + 1}列目`), x.name ? h('span', { class: 'col-item-name keep' }, x.name) : null),
        h('p', { class: 'col-item-role' }, roles.length ? [h('span', { class: 'col-arrow', 'aria-hidden': 'true' }, '→'), h('span', { class: 'visually-hidden' }, '役目: '), roles.map((t) => h('span', { class: 'badge badge--ok col-tag' }, t))] : h('span', { class: 'muted' }, '使わない')),
        // 値は1つずつ折らない（「2026年10月／3日」と値の途中で割れた）。値と値の間だけで折り返す。
        examples.length ? h('p', { class: 'col-item-example' }, '例: ', examples.map((v, i) => [i ? '　' : null, phrase(v)])) : null,
      );
    }),
  );
}

/**
 * 見本を描く。広い幅の表と、狭い幅の一覧を両方作り、CSS で片方だけを見せる（screens-import.css）。
 * @param {object} up サーバーの応え（upload）
 * @param {Array<{ value: number, name: string }>} opts 列の選択肢（import-map.js の columnOptions）
 * @param {object} cur いまの対応
 */
export function sampleView(up, opts, cur) {
  // 説明は、見えている形に合わせて分ける。一覧は列ごとに空でない値を3つまで拾うので、行のまとまりは残らない。
  return [
    h('p', { class: 'small muted sample-note sample-note--table' }, phrase(`ファイルの先頭の行を、中身のまま見せています（${Math.min(20, (up.sample || []).length)}行）。左の番号は、ファイルの何行目かです。`)),
    h('p', { class: 'small muted sample-note sample-note--list' }, phrase('列ごとに、空でない値を3つまで、中身のまま見せています。')),
    sampleTable(up, opts, cur),
    columnList(up, opts, cur),
  ];
}
