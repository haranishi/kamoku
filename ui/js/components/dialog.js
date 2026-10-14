// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ダイアログ。標準の <dialog> と showModal() に、フォーカスの閉じ込めとEscでの閉じ方を任せる。
// 閉じたら、開いたときにフォーカスがあった要素へ戻す。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';
import { button, iconButton } from './fields.js';

/**
 * @typedef {object} DialogAction
 * @property {string} label
 * @property {'primary'|'secondary'|'danger'|'ghost'} [variant]
 * @property {string} [kbd]
 * @property {boolean} [autofocus] 開いたときに、このボタンにフォーカスを置く（本文に入力欄があれば、そちらが先）
 * @property {(ctrl: ModalControl) => void|Promise<void>} onClick 閉じるかどうかは、この関数が ctrl.close() で決める
 */

/**
 * @typedef {object} ModalControl
 * @property {HTMLDialogElement} el
 * @property {HTMLElement} body
 * @property {(value?: string) => void} close
 * @property {(busy: boolean) => void} setBusy 送信中はボタンとEscを止める
 * @property {(message?: string) => void} setError 下のボタンの上に、原因と直し方を出す
 * @property {(node: Node) => void} setBody
 * @property {(text: string) => void} setTitle
 */

/**
 * ダイアログを開く。
 * 例: openModal({ title: 'テンプレートに登録', body, actions: [{ label: 'やめる', onClick: (c) => c.close() }] })
 * @param {{ title: string, body?: Node, actions?: DialogAction[], className?: string|string[], onClose?: (value: string) => void,
 *           closeOnBackdrop?: boolean, initialFocus?: HTMLElement, labelExtra?: Node }} opts
 * @returns {ModalControl}
 */
export function openModal(opts) {
  const opener = document.activeElement;
  const titleId = uid('dlg-title');
  const titleEl = h('h2', { id: titleId }, opts.title);
  const bodyEl = h('div', { class: 'dialog-body' }, opts.body || null);
  const errorEl = h('p', { class: 'field-error', role: 'alert' });
  let busy = false;
  const actionButtons = (opts.actions || []).map((a) =>
    button({ label: a.label, variant: a.variant || 'secondary', kbd: a.kbd, onClick: () => a.onClick(ctrl) }),
  );
  const foot = actionButtons.length || opts.footExtra ? h('div', { class: 'dialog-foot' }, opts.footExtra || null, actionButtons) : null;
  const closeBtn = iconButton({ iconName: 'close', label: '閉じる', onClick: () => ctrl.close('cancel') });
  const dlg = h(
    'dialog',
    { class: ['dialog', opts.className], 'aria-labelledby': titleId },
    h('div', { class: 'dialog-head' }, h('div', { class: 'page-head-text' }, titleEl, opts.labelExtra || null), closeBtn),
    bodyEl,
    h('div', { class: 'dialog-error' }, errorEl),
    foot,
  );

  dlg.addEventListener('cancel', (e) => {
    if (busy) e.preventDefault();
  });
  dlg.addEventListener('close', () => {
    dlg.remove();
    if (opener && typeof opener.focus === 'function' && document.contains(opener)) opener.focus({ preventScroll: true });
    if (opts.onClose) opts.onClose(dlg.returnValue);
  });
  if (opts.closeOnBackdrop) {
    // 背景（ダイアログの外側）を押したら閉じる。中身を押したときは閉じない。
    dlg.addEventListener('click', (e) => {
      if (e.target !== dlg || busy) return;
      const r = dlg.getBoundingClientRect();
      const inside = e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom;
      if (!inside) ctrl.close('cancel');
    });
  }

  // 最初のフォーカス: 指定 → 本文の最初の入力欄 → autofocus の付いたボタン → 最初のボタン（閉じるの×には置かない）。
  const auto = (opts.actions || []).findIndex((a) => a.autofocus);
  let first = opts.initialFocus || bodyEl.querySelector('input, select, textarea') || actionButtons[auto] || actionButtons[0] || null;
  if (!first) {
    // 押すものが無い開き方（詳細の引き出しなど）は、見出しに置く。読み上げが題名から始まる。
    titleEl.setAttribute('tabindex', '-1');
    first = titleEl;
  }
  first.setAttribute('autofocus', '');

  /** @type {ModalControl} */
  const ctrl = {
    el: dlg,
    body: bodyEl,
    close(value = 'done') {
      if (dlg.open) dlg.close(value);
    },
    setBusy(b) {
      busy = b;
      actionButtons.forEach((btn) => {
        btn.disabled = b;
      });
      closeBtn.disabled = b;
      dlg.setAttribute('aria-busy', b ? 'true' : 'false');
    },
    setError(message) {
      errorEl.replaceChildren();
      if (message) errorEl.append(icon('alert', { size: 16 }), h('span', null, message));
    },
    setBody(node) {
      bodyEl.replaceChildren(node);
    },
    setTitle(text) {
      titleEl.textContent = text;
    },
  };

  document.body.append(dlg);
  dlg.showModal();
  return ctrl;
}

/**
 * はい・いいえを聞くダイアログ。押した結果を返す。
 * 例: if (await confirmDialog({ title: '入力を捨てますか', confirmLabel: '捨てる' })) ...
 * @param {{ title: string, message?: string|Node, confirmLabel?: string, cancelLabel?: string, danger?: boolean }} opts
 * @returns {Promise<boolean>}
 */
export function confirmDialog(opts) {
  return new Promise((resolve) => {
    let answered = false;
    openModal({
      title: opts.title,
      body: opts.message ? h('div', null, opts.message) : null,
      actions: [
        {
          label: opts.cancelLabel || 'やめる',
          onClick: (c) => c.close('cancel'),
        },
        {
          label: opts.confirmLabel || 'はい',
          variant: opts.danger ? 'danger' : 'primary',
          onClick: (c) => {
            answered = true;
            c.close('ok');
          },
        },
      ],
      onClose: () => resolve(answered),
    });
  });
}
