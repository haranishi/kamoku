// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ショートカットの一覧（? キー。AC-10.2）。中身は keyboard.md と同じ。

import { h } from '../dom.js';
import { modLabel } from '../keys.js';
import { openModal } from './dialog.js';

function section(title, rows) {
  return h(
    'section',
    { class: 'keys-section' },
    h('h3', null, title),
    h(
      'div',
      { class: 'table-wrap' },
      h(
        'table',
        { class: 'table keys-table' },
        h('caption', null, `${title}のショートカット`),
        h('thead', null, h('tr', null, h('th', { scope: 'col' }, 'キー'), h('th', { scope: 'col' }, '動き'))),
        h(
          'tbody',
          null,
          rows.map(([keys, what]) =>
            h('tr', null, h('th', { scope: 'row' }, keys.map((k, i) => [i ? ' ' : '', h('kbd', null, k)])), h('td', { class: 'wrap' }, what)),
          ),
        ),
      ),
    ),
  );
}

/** ショートカットの一覧を開く。 */
export function openShortcutsHelp() {
  const mod = modLabel();
  const body = h(
    'div',
    { class: 'keys-help' },
    h('p', { class: 'muted small' }, '文字を打つ欄にいるとき（変換中を含む）は、1文字のキーは働きません。'),
    section('全体', [
      [[mod, 'K'], 'コマンド一覧（画面の移動、主な操作、科目と仕訳を探す）'],
      [['g', 'h'], 'ホームへ'],
      [['g', 'n'], '取引の登録へ（n だけでも同じ）'],
      [['g', 'i'], '未確認へ'],
      [['g', 'j'], '仕訳帳へ'],
      [['g', 'r'], '帳票へ'],
      [['g', 'w'], '口座へ'],
      [['g', 's'], '設定へ'],
      [['/'], '一覧の検索欄へ'],
      [['?'], 'この一覧'],
      [['Esc'], '閉じる、取り消す、候補を閉じる'],
    ]),
    section('取引の登録', [
      [['1', '2', '3'], '種別の欄で、支出・収入・振替を選ぶ'],
      [['Tab'], '次の欄へ（Enterも次の欄へ進む）'],
      [[mod, 'Enter'], '保存して続ける（最後の欄のEnterでも保存）'],
      [['t'], '日付の欄で今日にする'],
      [['↑', '↓'], '日付の欄で1日ずつ動かす（+ と - も同じ）'],
    ]),
    section('仕訳帳', [
      [['j', 'k'], '下の行、上の行へ（上下の矢印も同じ）'],
      [['Enter'], '詳細を開く'],
      [['e'], '訂正する'],
      [['v'], '取り消す'],
      [['[', ']'], '前の月、次の月へ'],
    ]),
    section('未確認', [
      [['Enter'], '行を確定候補にして、次の行の科目へ（科目の候補が開いていれば、選んでから進む）'],
      [[mod, 'Enter'], '確定候補を確定する（先に件数と、入金・出金の合計の確認が出る）'],
      [['↑', '↓'], '同じ列の、上の行・下の行へ（候補の一覧が閉じているとき）'],
      [['Esc'], '候補の一覧を閉じる。閉じていれば、行の印（候補の欄）へ'],
      [['x'], '取り込まない（行の印にいるとき）'],
      [['Shift', 'Space'], '確定候補の印を付ける、外す（取引先とメモの欄では空白を打つ）'],
    ]),
    section('帳票', [
      [['[', ']'], '前の期間、次の期間へ（年・月・日付）'],
    ]),
  );
  return openModal({
    title: 'ショートカット',
    body,
    actions: [{ label: '閉じる', variant: 'primary', onClick: (c) => c.close() }],
  });
}
