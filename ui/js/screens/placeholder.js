// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// どの経路にも当たらないアドレスの画面（「ページが見つかりません」）。U1 のときは準備中の画面にも使っていた。

import { h } from '../dom.js';
import { emptyState } from '../components/empty.js';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const notFound = Boolean(ctx.route.notFound);
  root.append(
    h('div', { class: 'page-head' }, h('h1', { tabindex: '-1' }, ctx.route.title)),
    h(
      'div',
      { class: 'card' },
      emptyState({
        iconName: notFound ? 'alert' : 'clock',
        title: notFound ? 'このアドレスの画面はありません' : 'この画面は準備中です',
        body: notFound
          ? 'アドレスを確かめるか、ホームに戻ってください。'
          : 'いま使えるのは「ホーム」「取引を登録」「仕訳帳」です。次の版で、この画面を足します。',
        actions: [
          { label: 'ホームへ', href: '#/' },
          { label: '取引を登録', href: '#/entry', primary: true },
        ],
      }),
    ),
  );
  return () => {};
}
