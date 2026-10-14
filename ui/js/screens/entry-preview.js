// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳のプレビュー。サーバーの計算（POST /entries/preview）の結果を、そのまま見せる（AC-02.2 の画面側）。
// 画面で計算しないのは、見えている仕訳と保存される仕訳がずれる余地を消すため（ux-five-layers.md の決定3）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';

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
            h('td', { class: 'pv-account' }, h('span', null, l.account_name), l.memo ? h('span', { class: 'sub' }, l.memo) : null),
            h('td', { class: 'num' }, formatYen(l.amount), l.tax_amount ? h('span', { class: 'sub' }, `（税${formatYen(l.tax_amount)}）`) : null),
          ),
        ),
      ),
      h('tfoot', null, h('tr', null, h('th', { scope: 'row', colspan: '2' }, '合計（借方と貸方で同じ）'), h('td', { class: 'num' }, formatYen(total)))),
    ),
  );
}

/**
 * プレビューの入れ物。状態は「待ち」「表示」「入力の誤り」「通信の失敗」の4つ。
 * @returns {{ el: HTMLElement, idle: (text: string) => void, show: (res: { lines: any[], warnings: string[] }) => void,
 *            invalid: (message: string) => void, failed: (message: string, retry: () => void) => void, errorId: string }}
 */
export function createPreviewPanel() {
  const body = h('div', { class: 'pv-body', 'aria-live': 'polite' });
  const errorId = 'preview-error';
  const el = h('section', { class: 'card pv-card', 'aria-labelledby': 'pv-title' }, h('h2', { id: 'pv-title', class: 'card-title' }, '仕訳のプレビュー'), body);
  return {
    el,
    errorId,
    idle(text) {
      replace(body, h('p', { class: 'muted pv-idle' }, text));
    },
    show(res) {
      replace(body, 
        previewTable(res.lines),
        res.warnings && res.warnings.length
          ? h('ul', { class: 'pv-warnings' }, res.warnings.map((w) => h('li', null, icon('alert', { size: 16 }), h('span', null, w))))
          : null,
      );
    },
    invalid(message) {
      replace(body, h('p', { class: 'pv-note' }, icon('info', { size: 16 }), h('span', null, message)));
    },
    failed(message, retry) {
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
