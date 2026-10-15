// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// タブ（ARIAの tablist）。左右の矢印・Home・End で動き、動いたタブをそのまま選ぶ。
// 中身の入れ物（tabpanel）は呼び出し側が持ち、panel を渡すと aria の結び付けをここで行う。

import { h, uid } from '../dom.js';

/**
 * 横に入りきらないタブの並びに、続きのある側の印（data-more="left right"）を付ける。CSS がその側の端をぼかす
 * （次のタブの文字が途中で切れて見えるだけでは、横に続くと気づきにくかった）。端から1px以内は、端に着いたとみなす。
 */
function overflowSides(scrollLeft, scrollWidth, clientWidth) {
  const max = scrollWidth - clientWidth;
  if (max <= 1) return '';
  return [scrollLeft > 1 && 'left', scrollLeft < max - 1 && 'right'].filter(Boolean).join(' ');
}

function markOverflow(el) {
  const sides = overflowSides(el.scrollLeft, el.scrollWidth, el.clientWidth);
  if (sides) {
    if (el.dataset.more !== sides) el.dataset.more = sides;
  } else if (el.dataset.more) {
    delete el.dataset.more;
  }
}

// 大きさの変化は、すべてのタブの並びを1つの見張りで見る（並びが文書から外れたら、見張りから外す）。
const resized = typeof ResizeObserver === 'function'
  ? new ResizeObserver((entries) => {
      for (const { target } of entries) {
        if (target.isConnected) markOverflow(target);
        else resized.unobserve(target);
      }
    })
  : null;

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
  el.addEventListener('scroll', () => markOverflow(el), { passive: true });
  if (resized) resized.observe(el);
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
