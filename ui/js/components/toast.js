// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 通知。入れ物は aria-live="polite"（index.html の #toasts）。フォーカスは奪わない。
// 成功は4秒で消える（読み上げの途中で消えないよう、マウスやフォーカスが乗っている間は待つ）。
// エラーは role="alert" で、利用者が閉じるまで消さない（screens-other.md の共通の部品）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { iconButton } from './fields.js';

const SUCCESS_MS = 4000;
const MAX_VISIBLE = 4;
let region = null;

/** 入れ物を決める。main.js が起動時に呼ぶ。 */
export function initToasts(el) {
  region = el;
}

function actionEl(action, dismiss) {
  if (!action) return null;
  if (action.href) {
    return h('a', { href: action.href, class: 'link-btn', onclick: () => dismiss() }, action.label);
  }
  return h(
    'button',
    {
      type: 'button',
      class: 'link-btn',
      onclick: () => {
        dismiss();
        action.onClick();
      },
    },
    action.label,
  );
}

function show(kind, message, { action, detail } = {}) {
  if (!region) return () => {};
  let timer = null;
  let hovered = false;
  // 入れ物が aria-live="polite" なので、成功には役割を付けない（二重に読まれないように）。エラーだけ割り込みで読む。
  const item = h('div', { class: ['toast', `toast--${kind}`], role: kind === 'error' ? 'alert' : null });
  const dismiss = () => {
    clearTimeout(timer);
    item.remove();
  };
  item.append(
    icon(kind === 'error' ? 'alert' : kind === 'success' ? 'check-circle' : 'info', { className: 'toast-icon' }),
    h(
      'div',
      { class: 'toast-body' },
      h('p', null, message),
      detail ? h('p', { class: 'small muted' }, detail) : null,
      action ? h('div', { class: 'toast-actions' }, actionEl(action, dismiss)) : null,
    ),
    iconButton({ iconName: 'close', label: 'この通知を閉じる', onClick: dismiss }),
  );
  if (kind !== 'error') {
    const arm = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (!hovered && !item.contains(document.activeElement)) dismiss();
        else arm();
      }, SUCCESS_MS);
    };
    item.addEventListener('mouseenter', () => {
      hovered = true;
    });
    item.addEventListener('mouseleave', () => {
      hovered = false;
    });
    arm();
  }
  region.append(item);
  // 古い成功の通知から消して、画面を埋めないようにする。エラーは残す。
  const items = [...region.children];
  if (items.length > MAX_VISIBLE) {
    const old = items.find((x) => !x.classList.contains('toast--error'));
    if (old) old.remove();
  }
  return dismiss;
}

/**
 * 例: toast.success('No.98 を登録しました', { action: { label: '仕訳を見る', href: '#/journal/e_..' } })
 *     toast.error(err.message)
 */
export const toast = {
  success: (message, opts) => show('success', message, opts),
  info: (message, opts) => show('info', message, opts),
  error: (message, opts) => show('error', message, opts),
};
