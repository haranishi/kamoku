// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 日付の欄（keyboard.md の「取引の登録」）。「1005」「10/5」「20261005」「t」を受け付け、離れたら「2026-10-05 (月)」と見せる。
// 上下の矢印と + - で1日ずつ動かす。「-」は「10-5」の区切りにも使うので、欄の文字を全部選んでいるときだけ1日戻す。

import { h, selectAllOnFocus } from '../dom.js';
import { shouldIgnoreKey } from '../ime.js';
import { parseDateInput, addDays, todayIso } from '../input-parse.js';
import { formatDate } from '../format.js';

/**
 * 例: const d = createDateInput({ value: '2026-10-14', onChange: (iso) => ..., onError: (msg) => field.setError(msg) })
 * @param {{ id?: string, value?: string|null, onChange?: (iso: string|null) => void, onError?: (message: string) => void,
 *           today?: () => string, arrowKeys?: boolean, display?: (iso: string) => string, placeholder?: string }} opts
 *   arrowKeys: 偽なら上下の矢印で日を動かさず、外へ回す（未確認の表で、隣の行へ移るため。+ と - は効く）。
 *   display: 欄を離れたときの見せ方（既定は「2026-10-05 (月)」）。
 */
export function createDateInput(opts = {}) {
  const today = opts.today || (() => todayIso());
  const arrowKeys = opts.arrowKeys !== false;
  const display = opts.display || formatDate;
  let value = opts.value || null;
  const input = h('input', {
    type: 'text',
    id: opts.id,
    class: ['input', opts.inputClass],
    autocomplete: 'off',
    spellcheck: 'false',
    enterkeyhint: 'next',
    placeholder: opts.placeholder ?? '例: 1005、10/5、t（今日）',
  });

  const show = () => {
    input.value = value ? display(value) : '';
  };

  function set(iso, notify = true) {
    value = iso;
    if (document.activeElement === input) {
      input.value = iso || '';
      input.select();
    } else {
      show();
    }
    if (opts.onError) opts.onError('');
    if (notify && opts.onChange) opts.onChange(value);
  }

  /** 欄の文字を読み直す。読めれば値にして真を返す。空や読めないときは理由を onError に渡して偽。 */
  function commit() {
    const text = input.value.trim();
    if (value && (text === display(value) || text === formatDate(value) || text === value)) return true;
    const r = parseDateInput(text, { today: today(), year: value ? Number(value.slice(0, 4)) : undefined });
    if (r.ok) {
      set(r.value);
      return true;
    }
    if (opts.onError) opts.onError(r.error);
    if (value !== null) {
      value = null;
      if (opts.onChange) opts.onChange(null);
    }
    return false;
  }

  const allSelected = () => input.selectionStart === 0 && input.selectionEnd === input.value.length;

  // 欄に入ったら、整形した表示（曜日つき）を、打ち直しやすい YYYY-MM-DD に戻してから全部を選ぶ。
  input.addEventListener('focus', () => {
    if (value) input.value = value;
  });
  selectAllOnFocus(input);
  input.addEventListener('blur', () => {
    if (input.value.trim() === '') {
      if (opts.onError) opts.onError('日付を入れてください。');
      return;
    }
    if (commit()) show();
  });
  input.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.altKey || e.ctrlKey || e.metaKey) return;
    const base = value || today();
    if ((arrowKeys && e.key === 'ArrowUp') || (e.key === '+' && allSelected())) {
      e.preventDefault();
      set(value ? addDays(base, 1) : base);
    } else if ((arrowKeys && e.key === 'ArrowDown') || (e.key === '-' && allSelected())) {
      e.preventDefault();
      set(value ? addDays(base, -1) : base);
    } else if ((e.key === 't' || e.key === 'T') && (allSelected() || input.value === '')) {
      e.preventDefault();
      set(today());
    } else if ((e.key === 'y' || e.key === 'Y') && (allSelected() || input.value === '')) {
      e.preventDefault();
      set(addDays(today(), -1));
    }
  });

  show();
  return {
    el: input,
    input,
    get value() {
      return value;
    },
    setValue(iso) {
      set(iso, false);
    },
    commit,
  };
}
