// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// タブ（ARIAの tablist）。左右の矢印・Home・End で動き、動いたタブをそのまま選ぶ。
// 中身の入れ物（tabpanel）は呼び出し側が持ち、panel を渡すと aria の結び付けをここで行う。

import { h, uid } from '../dom.js';

/**
 * 例: const t = createTabs({ label: '入力の形', tabs: [{ id: 'simple', label: 'かんたん' }, ...], selected: 'simple', onChange })
 *     root.append(t.el, t.panel)
 * @param {{ label: string, tabs: Array<{ id: string, label: string, disabled?: boolean }>, selected: string,
 *           onChange?: (id: string) => void }} opts
 */
export function createTabs(opts) {
  const base = uid('tabs');
  let selected = opts.selected;
  const panel = h('div', { role: 'tabpanel', id: `${base}-panel`, class: 'tabpanel' });
  const buttons = opts.tabs.map((t) =>
    h(
      'button',
      {
        type: 'button',
        role: 'tab',
        class: 'tab',
        id: `${base}-${t.id}`,
        'aria-controls': panel.id,
        disabled: t.disabled || false,
        dataset: { tab: t.id },
        onclick: () => select(t.id, true),
      },
      t.label,
    ),
  );
  const el = h('div', { role: 'tablist', 'aria-label': opts.label, class: 'tabs' }, buttons);

  function sync() {
    buttons.forEach((b) => {
      const on = b.dataset.tab === selected;
      b.setAttribute('aria-selected', on ? 'true' : 'false');
      b.tabIndex = on ? 0 : -1;
      if (on) panel.setAttribute('aria-labelledby', b.id);
    });
  }

  function select(id, notify) {
    if (id === selected) return;
    selected = id;
    sync();
    if (notify && opts.onChange) opts.onChange(id);
  }

  el.addEventListener('keydown', (e) => {
    const enabled = buttons.filter((b) => !b.disabled);
    const i = enabled.indexOf(document.activeElement);
    if (i < 0) return;
    let next = null;
    if (e.key === 'ArrowRight') next = enabled[(i + 1) % enabled.length];
    else if (e.key === 'ArrowLeft') next = enabled[(i - 1 + enabled.length) % enabled.length];
    else if (e.key === 'Home') next = enabled[0];
    else if (e.key === 'End') next = enabled[enabled.length - 1];
    if (!next) return;
    e.preventDefault();
    next.focus();
    select(next.dataset.tab, true);
  });

  sync();
  return {
    el,
    panel,
    get selected() {
      return selected;
    },
    /** 選ぶタブを変える（onChange は呼ばない）。 */
    select: (id) => select(id, false),
  };
}
