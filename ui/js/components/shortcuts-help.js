// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ショートカットの一覧（? キー。AC-10.2）。中身は keyboard.md と同じ。
// 1文字のショートカットを切る欄も、ここに置く（WCAG 2.1.4。切っても、この一覧はサイドバーとコマンド一覧から開ける）。

import { h, uid } from '../dom.js';
import { modLabel, singleKeysEnabled, setSingleKeys, onSingleKeysChange } from '../keys.js';
import { openModal } from './dialog.js';
import { toast } from './toast.js';

/** 切り替えたことを、読み上げの領域（通知）で知らせる。 */
function announce() {
  toast.info(singleKeysEnabled() ? '1文字のショートカットを使います。' : '1文字のショートカットを切りました。');
}

/** 1文字のショートカットを、使う⇔切る に切り替えて知らせる（コマンド一覧から呼ぶ）。 */
export function toggleSingleKeys() {
  setSingleKeys(!singleKeysEnabled());
  announce();
}

/**
 * rows は [キー, 動き, 1文字のショートカットか]。3つ目が真の行は、切ってある間「いま切ってあります」を添える
 * （欄や行の中だけで働くキー、組み合わせのキー、Esc・Enter・矢印は、切っても働くので添えない）。
 */
function section(title, rows, offMarks) {
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
          rows.map(([keys, what, single]) => {
            const mark = single ? h('span', { class: 'badge badge--muted keys-off-mark', hidden: true }, 'いま切ってあります') : null;
            if (mark) offMarks.push(mark);
            return h('tr', null, h('th', { scope: 'row' }, keys.map((k, i) => [i ? ' ' : '', h('kbd', null, k)])), h('td', { class: 'wrap' }, what, mark));
          }),
        ),
      ),
    ),
  );
}

/** 一覧のいちばん上の、1文字のショートカットを使うかの欄。 */
function singleKeysToggle(mod) {
  const hintId = uid('keys-single-hint');
  const box = h('input', { type: 'checkbox', 'aria-describedby': hintId, dataset: { singleKeys: '' } });
  box.checked = singleKeysEnabled();
  box.addEventListener('change', () => {
    setSingleKeys(box.checked);
    announce();
  });
  const el = h(
    'div',
    { class: 'keys-toggle' },
    h('label', { class: 'check' }, box, h('span', null, '1文字のショートカットを使う')),
    h(
      'p',
      { class: 'field-hint', id: hintId },
      `切ると、画面のどこでも働く1文字のキー（n・?・/・g＋文字、仕訳帳の j k e v など）を止めます。${mod}K・${mod}Enter・Esc と、欄や行の中だけで働くキー（日付の t、未確認の x など）は使えます。`,
    ),
  );
  return { el, box };
}

/** ショートカットの一覧を開く。 */
export function openShortcutsHelp() {
  const mod = modLabel();
  const offMarks = [];
  const toggle = singleKeysToggle(mod);
  const body = h(
    'div',
    { class: 'keys-help' },
    toggle.el,
    h('p', { class: 'muted small' }, '文字を打つ欄にいるとき（変換中を含む）は、1文字のキーは働きません。'),
    section(
      '全体',
      [
        [[mod, 'K'], 'コマンド一覧（画面の移動、主な操作、科目と仕訳を探す）'],
        [['g', 'h'], 'ホームへ', true],
        [['g', 'n'], '取引の登録へ（n だけでも同じ）', true],
        [['g', 'i'], '未確認へ', true],
        [['g', 'j'], '仕訳帳へ', true],
        [['g', 'r'], '帳票へ', true],
        [['g', 'w'], '口座へ', true],
        [['g', 's'], '設定へ', true],
        [['/'], '一覧の検索欄へ', true],
        [['?'], 'この一覧', true],
        [['Esc'], '閉じる、取り消す、候補を閉じる'],
      ],
      offMarks,
    ),
    section(
      '取引の登録',
      [
        [['1', '2', '3'], '種別の欄で、支出・収入・振替を選ぶ'],
        [['Tab'], '次の欄へ（Enterも次の欄へ進む）'],
        [[mod, 'Enter'], '保存して続ける（最後の欄のEnterでも保存）'],
        [['t'], '日付の欄で今日にする'],
        [['↑', '↓'], '日付の欄で1日ずつ動かす（+ と - も同じ）'],
      ],
      offMarks,
    ),
    section(
      '仕訳帳',
      [
        [['j', 'k'], '下の行、上の行へ（上下の矢印も同じ）', true],
        [['Enter'], '詳細を開く'],
        [['e'], '訂正する', true],
        [['v'], '取り消す', true],
        [['[', ']'], '前の月、次の月へ', true],
      ],
      offMarks,
    ),
    section(
      '未確認',
      [
        [['Enter'], '行を確定する行にして、次の行の科目へ（科目の候補が開いていれば、選んでから進む）'],
        [[mod, 'Enter'], '確定する行を確定する（先に件数と、入金・出金の合計の確認が出る）'],
        [['↑', '↓'], '同じ列の、上の行・下の行へ（候補の一覧が閉じているとき）。税区分の欄では、行を動かさずに税区分を切り替える'],
        [['Esc'], '候補の一覧を閉じる。閉じていれば、行の印へ。検索の欄では、欄から抜ける'],
        [['x'], '無視する（行の印か「無視」のボタンにいるとき）'],
        [[mod, 'Z'], '最後に無視した行を戻す（文字の欄にいるときは、ブラウザの標準の取り消し）'],
        [['Shift', 'Space'], '確定する行の印を付ける、外す（取引先とメモの欄では空白を打つ）'],
      ],
      offMarks,
    ),
    section(
      '帳票',
      [
        [['[', ']'], '前の期間、次の期間へ（年・月・日付）', true],
        [['j', 'k'], '総勘定元帳で、下の行、上の行へ', true],
      ],
      offMarks,
    ),
  );
  // コマンド一覧からも切り替えられるので、開いている間は欄と印を、いまの設定に合わせ続ける。
  const sync = (on) => {
    toggle.box.checked = on;
    for (const m of offMarks) m.hidden = on;
  };
  sync(singleKeysEnabled());
  const stop = onSingleKeysChange(sync);
  return openModal({
    title: 'ショートカット',
    body,
    actions: [{ label: '閉じる', variant: 'primary', onClick: (c) => c.close() }],
    onClose: stop,
  });
}
