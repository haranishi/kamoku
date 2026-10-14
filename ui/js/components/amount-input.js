// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 金額の欄。全角の数字、カンマ、「1000+650」のような四則を受け付ける（AC-02.3）。eval は使わない。
// 打っている間は黙って読み（プレビューのため）、離れたときに「11,000」と整形して、式なら「1000+650 = 1,650円」を添える。

import { h, selectAllOnFocus } from '../dom.js';
import { parseAmountExpr } from '../input-parse.js';
import { formatYen } from '../format.js';

/**
 * 例: const a = createAmountInput({ onChange: (n) => ..., onError: (m) => f.setError(m), onNote: (t) => f.setHint(t) })
 * @param {{ id?: string, value?: number|null, onChange?: (n: number|null) => void, onError?: (message: string) => void,
 *           onNote?: (text: string) => void, label?: string }} opts
 */
export function createAmountInput(opts = {}) {
  let value = opts.value ?? null;
  const input = h('input', {
    type: 'text',
    id: opts.id,
    class: 'input input--num',
    autocomplete: 'off',
    spellcheck: 'false',
    inputmode: 'decimal',
    enterkeyhint: 'next',
    placeholder: '例: 1650、1000+650',
  });
  if (value !== null) input.value = formatYen(value);

  function update(next) {
    if (next === value) return;
    value = next;
    if (opts.onChange) opts.onChange(value);
  }

  /** 欄の文字を読み直す。読めれば整形して真。読めなければ理由を onError に渡して偽。 */
  function commit() {
    const r = parseAmountExpr(input.value);
    if (!r.ok) {
      update(null);
      if (opts.onError) opts.onError(r.error);
      return false;
    }
    const raw = input.value.trim();
    update(r.value);
    input.value = formatYen(r.value);
    if (opts.onError) opts.onError('');
    if (opts.onNote) {
      const parts = [];
      if (r.isExpr) parts.push(`${raw} = ${formatYen(r.value)}円`);
      if (r.rounded) parts.push('割り算の端数は切り捨てました');
      opts.onNote(parts.join('。'));
    }
    return true;
  }

  selectAllOnFocus(input);
  input.addEventListener('input', () => {
    const r = parseAmountExpr(input.value);
    update(r.ok ? r.value : null);
    if (opts.onNote) opts.onNote('');
  });
  input.addEventListener('blur', () => {
    if (input.value.trim() === '') return;
    commit();
  });

  return {
    el: input,
    input,
    get value() {
      return value;
    },
    setValue(n) {
      value = n ?? null;
      input.value = value === null ? '' : formatYen(value);
      if (opts.onNote) opts.onNote('');
    },
    commit,
  };
}
