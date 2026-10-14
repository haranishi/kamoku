// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 引き出し（右から出る <dialog>）。仕訳の詳細などに使う。左から出すとメニューになる（768px未満）。
// 中身は openModal と同じで、背景を押しても閉じる。

import { openModal } from './dialog.js';

/**
 * 引き出しを開く。
 * 例: const d = openDrawer({ title: 'No.97 の仕訳', body, onClose: () => router.navigate('#/journal') });
 * @param {{ title: string, body?: Node, actions?: import('./dialog.js').DialogAction[], onClose?: (value: string) => void,
 *           side?: 'right'|'left', className?: string, labelExtra?: Node, initialFocus?: HTMLElement }} opts
 * @returns {import('./dialog.js').ModalControl}
 */
export function openDrawer(opts) {
  return openModal({
    ...opts,
    className: ['drawer', opts.side === 'left' && 'drawer--left', opts.className],
    closeOnBackdrop: true,
  });
}
