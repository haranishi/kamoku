// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 空の状態。何も無い理由と、次の一手を出す（screens-other.md の共通の部品）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { kbd, shortcutKeys } from './fields.js';

/**
 * 例: emptyState({ title: '最初の取引を登録しましょう', body: '...', actions: [{ label: '取引を登録', href: '#/entry', primary: true }] })
 * @param {{ title: string, body?: string, iconName?: string,
 *           actions?: Array<{ label: string, href?: string, onClick?: () => void, primary?: boolean, kbd?: string }> }} opts
 */
export function emptyState({ title, body, iconName = 'info', actions = [] }) {
  return h(
    'div',
    { class: 'empty' },
    h('span', { class: 'empty-icon' }, icon(iconName)),
    h('p', { class: 'empty-title' }, title),
    body ? h('p', { class: 'empty-body' }, body) : null,
    actions.length
      ? h(
          'div',
          { class: 'empty-actions' },
          actions.map((a) =>
            a.href
              ? h('a', { href: a.href, class: ['btn', a.primary && 'btn--primary'], 'aria-keyshortcuts': a.kbd ? shortcutKeys(a.kbd) : null }, a.label, a.kbd ? kbd(a.kbd) : null)
              : h('button', { type: 'button', class: ['btn', a.primary && 'btn--primary'], onclick: a.onClick }, a.label),
          ),
        )
      : null,
  );
}
