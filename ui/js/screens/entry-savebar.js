// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の、保存のボタンの帯。保存できない理由（読み取り専用・プレビューの失敗・貸借の不一致）があれば、
// ボタンを止めて、その理由を文字で出し、aria-describedby で結ぶ（読み上げでも止まった訳が分かるように）。

import { h } from '../dom.js';
import { modLabel } from '../keys.js';
import { button } from '../components/fields.js';

const NOTE_ID = 'entry-block-note';

/**
 * @param {{ correcting: boolean, onSave: (o: { cont: boolean }) => void, onCancel: () => void, onTemplate: () => void }} opts
 */
export function createSaveBar(opts) {
  const formError = h('p', { class: 'field-error form-error', role: 'alert' });
  const blockNote = h('p', { class: 'field-hint block-note', id: NOTE_ID });
  const saveCont = button({ label: opts.correcting ? '訂正を確かめる' : '保存して続ける', variant: 'primary', kbd: `${modLabel()} Enter`, onClick: () => opts.onSave({ cont: true }) });
  const saveOnly = opts.correcting ? null : button({ label: '保存', onClick: () => opts.onSave({ cont: false }) });
  const cancelBtn = button({ label: 'やめる', variant: 'ghost', kbd: 'Esc', onClick: () => opts.onCancel() });
  const tplSave = opts.correcting ? null : button({ label: 'テンプレートに登録', iconName: 'bookmark', small: true, variant: 'ghost', onClick: () => opts.onTemplate() });
  const el = h(
    'section',
    { class: 'card entry-actions-card', 'aria-label': '保存' },
    formError,
    h('div', { class: 'entry-actions' }, h('div', { class: 'entry-actions-main' }, saveCont, saveOnly, cancelBtn), tplSave),
    blockNote,
  );
  return {
    el,
    /** ボタンの上に、保存できなかった原因と直し方を出す（空なら消す）。 */
    setError(message) {
      formError.textContent = message || '';
    },
    /**
     * ボタンの状態を決める。reason があれば止めて、その理由を出す。
     * @param {{ reason: string, readOnly: boolean, busy: boolean }} p
     */
    update({ reason, readOnly, busy }) {
      blockNote.textContent = reason;
      for (const b of [saveCont, saveOnly, tplSave]) {
        if (!b) continue;
        b.disabled = Boolean(reason) || busy;
        if (reason) b.setAttribute('aria-describedby', readOnly ? `banner-readonly ${NOTE_ID}` : NOTE_ID);
        else b.removeAttribute('aria-describedby');
      }
    },
    /** テンプレートに登録できるのは、かんたん入力のときだけ。 */
    showTemplate(on) {
      if (tplSave) tplSave.hidden = !on;
    },
  };
}
