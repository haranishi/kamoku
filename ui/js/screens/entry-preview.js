// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳のプレビュー。サーバーの計算（POST /entries/preview）の結果を、そのまま見せる（AC-02.2 の画面側）。
// 画面で計算しないのは、見えている仕訳と保存される仕訳がずれる余地を消すため（ux-five-layers.md の決定3）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { lineMemo } from './entry-hints.js';
import { phrase } from '../components/phrase.js';

/**
 * 明細の表。借方の行を先に、貸方の行を後に、1行ずつ並べる。
 * 借方と貸方を左右に並べる形は、細い列で科目の名前が途中で割れるので採らなかった。
 */
export function previewTable(lines, caption = '保存される仕訳') {
  const ordered = [...lines.filter((l) => l.side === 'debit'), ...lines.filter((l) => l.side === 'credit')];
  const total = lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0);
  return h(
    'div',
    { class: 'table-wrap pv-wrap' },
    h(
      'table',
      { class: 'table pv-table' },
      h('caption', null, caption),
      h('thead', null, h('tr', null, h('th', { scope: 'col' }, '貸借'), h('th', { scope: 'col' }, '科目'), h('th', { scope: 'col', class: 'num' }, '金額'))),
      h(
        'tbody',
        null,
        ordered.map((l) =>
          h(
            'tr',
            { class: l.side === 'credit' ? 'pv-credit' : 'pv-debit' },
            h('td', { class: 'pv-side' }, l.side === 'debit' ? '借方' : '貸方'),
            h('td', { class: 'pv-account' }, h('span', null, l.account_name), lineMemo(l) ? h('span', { class: 'sub' }, lineMemo(l)) : null),
            h('td', { class: 'num' }, formatYen(l.amount), l.tax_amount ? h('span', { class: 'sub' }, `（税${formatYen(l.tax_amount)}）`) : null),
          ),
        ),
      ),
      h('tfoot', null, h('tr', null, h('th', { scope: 'row', colspan: '2' }, '合計（借方と貸方で同じ）'), h('td', { class: 'num' }, formatYen(total)))),
    ),
  );
}

/**
 * 1行の要約: 「借方 通信費 11,000／貸方 サンプルカード 11,000」。同じ側に何行もあれば「通信費 ほか1行」と、その側の合計。
 * 狭い幅（640px未満）で、表の代わりに見せる（表は「くわしく」で開く）。
 */
export function previewSummary(lines) {
  const side = (s, label) => {
    const xs = lines.filter((l) => l.side === s);
    if (!xs.length) return null;
    const name = xs.length > 1 ? `${xs[0].account_name} ほか${xs.length - 1}行` : xs[0].account_name;
    const sum = xs.reduce((a, l) => a + l.amount, 0);
    return h('span', { class: 'pv-sum-side' }, h('span', { class: 'pv-sum-label' }, label), ' ', h('span', { class: 'pv-sum-name keep' }, name), ' ', h('span', { class: 'num' }, formatYen(sum)));
  };
  const debit = side('debit', '借方');
  const credit = side('credit', '貸方');
  return [debit, debit && credit ? h('span', { class: 'pv-sum-sep', 'aria-hidden': 'true' }, '／') : null, credit];
}

/**
 * プレビューの入れ物。状態は「待ち」「表示」「入力の誤り」「画面で止めた誤り」「通信の失敗」の5つ。
 * 表示のとき、狭い幅では1行の要約と「くわしく」の開閉を出し、表は開いたときだけ見せる（screens-entry.css）。
 * @returns {{ el: HTMLElement, idle: (text: string) => void, show: (res: { lines: any[], warnings: string[] }) => void,
 *            invalid: (message: string) => void, blocked: (message: string) => void,
 *            failed: (message: string, retry: () => void) => void, errorId: string }}
 */
export function createPreviewPanel() {
  const body = h('div', { class: 'pv-body', id: 'pv-body', 'aria-live': 'polite' });
  const errorId = 'preview-error';
  // 狭い幅では表を畳むので、要約の側も読み上げる（隠れた表の変化は読み上げられない）。
  const summary = h('p', { class: 'pv-summary', 'aria-live': 'polite' });
  const toggle = h('button', { type: 'button', class: 'btn btn--ghost btn--sm pv-toggle', 'aria-expanded': 'false', 'aria-controls': 'pv-body' }, h('span', null, 'くわしく'), icon('chevron-down', { size: 16 }));
  const el = h('section', { class: 'card pv-card', 'aria-labelledby': 'pv-title' }, h('div', { class: 'pv-head' }, h('h2', { id: 'pv-title', class: 'card-title' }, '仕訳のプレビュー'), toggle), summary, body);
  toggle.addEventListener('click', () => {
    const open = toggle.getAttribute('aria-expanded') !== 'true';
    toggle.setAttribute('aria-expanded', String(open));
    el.classList.toggle('is-open', open);
  });
  /** 表のある状態か（要約と開閉を出すか）。 */
  const hasLines = (on) => el.classList.toggle('has-lines', on);
  return {
    el,
    errorId,
    idle(text) {
      hasLines(false);
      replace(body, h('p', { class: 'muted pv-idle' }, text));
    },
    show(res) {
      hasLines(true);
      summary.replaceChildren(...previewSummary(res.lines));
      replace(body,
        previewTable(res.lines),
        res.warnings && res.warnings.length
          ? h('ul', { class: 'pv-warnings' }, res.warnings.map((w) => h('li', null, icon('alert', { size: 16 }), h('span', null, w))))
          : null,
      );
    },
    invalid(message) {
      hasLines(false);
      replace(body, h('p', { class: 'pv-note' }, icon('info', { size: 16 }), h('span', null, message)));
    },
    /** 画面で見つけた誤り（範囲外の割合など）。保存される仕訳を出さず、誤りを出す。 */
    blocked(message) {
      hasLines(false);
      replace(body, h('p', { class: 'field-error pv-blocked' }, icon('alert', { size: 16 }), h('span', null, phrase(message))));
    },
    failed(message, retry) {
      hasLines(false);
      replace(body,
        h(
          'div',
          { class: 'pv-failed', id: errorId, role: 'alert' },
          h('p', { class: 'field-error' }, icon('alert', { size: 16 }), h('span', null, `仕訳のプレビューを作れませんでした。${message} 保存は止めています。`)),
          h('button', { type: 'button', class: 'btn btn--sm', onclick: retry }, icon('refresh', { size: 16 }), 'もう一度試す'),
        ),
      );
    },
  };
}
