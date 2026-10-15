// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面の上の帯。読み取り専用（台帳の検証の失敗）とデモの表示に使う。色に加えて、記号と文で伝える。

import { h } from '../dom.js';
import { icon } from '../icons.js';

const ICONS = { danger: 'lock', warn: 'alert', info: 'info' };

/**
 * 例: banner({ kind: 'danger', title: '読み取り専用で開いています', body: '…' })
 * title と body は、文字列のほか、Node（またはその並び）（js/components/phrase.js の phrase()）も渡せる。
 * extra は本文のすぐ後に置く（言葉の説明など。<p> の外に置く要素）。
 * @param {{ kind: 'danger'|'warn'|'info', title: string|Node|Node[], body?: string|Node|Node[], extra?: Node, actions?: Node, id?: string }} opts
 */
export function banner({ kind, title, body, extra, actions, id }) {
  return h(
    'div',
    { class: ['banner', `banner--${kind}`], id, role: kind === 'danger' ? 'alert' : 'note' },
    icon(ICONS[kind] || 'info'),
    h('div', { class: 'banner-body' }, h('p', { class: 'banner-title' }, title), body ? h('p', null, body) : null, extra || null, actions || null),
  );
}
