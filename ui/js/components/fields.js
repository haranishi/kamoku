// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 入力欄とボタンの組み立て。見えるラベル、欄のすぐ下のエラー、aria-describedby の結び付けを、ここに集める（nfr.md）。

import { h, uid, addDescribedBy, removeDescribedBy } from '../dom.js';
import { icon } from '../icons.js';

/**
 * ラベル・入力・説明・エラーの一式を作る。
 * 例: const f = field({ label: '日付', control: h('input', { class: 'input' }) }); f.setError('日付を入れてください。');
 * @param {{ label: string, control: HTMLElement, id?: string, hint?: string|Node, required?: boolean,
 *           labelFor?: HTMLElement, className?: string, after?: Node, clearOnInput?: boolean }} opts
 *   labelFor: ラベルを結ぶ要素（候補欄など、control の中の input を指すとき）
 *   clearOnInput: 入力し直したら、その欄の誤りを消す（入力のたびに検査し直す欄では使わない）
 * @returns {{ el: HTMLElement, control: HTMLElement, hint: HTMLElement|null, setError: (msg?: string) => void,
 *             setHint: (text: string) => void, setLabel: (text: string) => void }}
 */
export function field({ label, control, id, hint, required = false, labelFor, className, after, clearOnInput = false }) {
  const target = labelFor || control;
  const controlId = target.id || id || uid('f');
  target.id = controlId;
  const hintEl = hint ? h('p', { class: 'field-hint', id: `${controlId}-hint` }, hint) : null;
  // 誤りの入れ物は、空のうちから読み上げの領域にしておく（欄を出た時点の誤りは、フォーカスが次の欄にあり、その場で読まれないため）。
  // 空のときも見えない形で置いておく（components.css）。表示しない入れ物に文を入れても、読まれないことがあるため。
  const errorEl = h('p', { class: 'field-error', id: `${controlId}-err`, 'aria-live': 'polite' });
  // 出している文。同じ文をもう一度入れても、要素を作り直さない（読み上げが同じ文を何度も繰り返さないように）。
  let shown = '';
  let errorText = null;
  if (hintEl) addDescribedBy(target, hintEl.id);
  // 必須は欄の状態として伝え、見える「必須」の札は読み上げない（名前が「日付必須」とつながって読まれたため）。
  if (required) target.setAttribute('aria-required', 'true');
  const labelText = h('span', null, label);
  const el = h(
    'div',
    { class: ['field', className] },
    h('label', { class: 'field-label', for: controlId }, labelText, required ? h('span', { class: 'req', 'aria-hidden': 'true' }, '必須') : null),
    control,
    after || null,
    hintEl,
    errorEl,
  );
  const f = {
    el,
    control,
    hint: hintEl,
    setLabel(text) {
      labelText.textContent = text;
    },
    setError(message) {
      const text = message ? String(message) : '';
      if (text) {
        target.setAttribute('aria-invalid', 'true');
        addDescribedBy(target, errorEl.id);
      } else {
        target.removeAttribute('aria-invalid');
        removeDescribedBy(target, errorEl.id);
      }
      if (text === shown) return;
      shown = text;
      // 文は1つの文字の節の中身だけを書き換える。保存の検査が「消してから入れ直す」ときに、同じ文なら、
      // 描く前に元どおりになり、読み上げの木も変わらない（作り直すと、新しく出た文として読み直される）。
      if (errorText) {
        errorText.data = text;
      } else if (text) {
        errorText = document.createTextNode(text);
        errorEl.append(icon('alert', { size: 16 }), h('span', { class: 'field-error-text' }, errorText));
      }
    },
    setHint(text) {
      if (hintEl) hintEl.textContent = text;
    },
  };
  // 直したのに、もう一度押すまで誤りの文と印が残っていたので、打ち直した時点で消す。
  if (clearOnInput) {
    el.addEventListener('input', () => {
      if (shown) f.setError('');
    });
  }
  return f;
}

/**
 * 文字の入力欄。日本語の候補や自動の修正を切ってある（金額や日付で邪魔になるため）。
 * @param {Record<string, any>} [props] h() に渡す属性
 */
export function textInput(props = {}) {
  const { class: cls, ...rest } = props;
  return h('input', { type: 'text', autocomplete: 'off', spellcheck: 'false', ...rest, class: ['input', cls] });
}

/**
 * 選択欄。options は [{ value, label }]。
 * @param {Record<string, any>} props
 * @param {Array<{ value: string, label: string }>} options
 */
export function selectInput(props, options) {
  const { value, class: cls, ...rest } = props;
  const el = h('select', { ...rest, class: ['select', cls] });
  for (const o of options) el.append(h('option', { value: o.value }, o.label));
  if (value !== undefined && value !== null) el.value = value;
  return el;
}

/**
 * キーの表示（「Ctrl」「Enter」など）。目で見る手がかりなので、読み上げからは外す（ボタンの名前に「保存 ⌘ Enter」と
 * キーが混ざって読まれたため）。キーは、押す要素の aria-keyshortcuts で伝える（shortcutKeys）。
 */
export function kbd(text) {
  return h('kbd', { 'aria-hidden': 'true' }, text);
}

const KEY_NAMES = { Ctrl: 'Control', '⌘': 'Meta', Cmd: 'Meta', Esc: 'Escape', '⌥': 'Alt', '⇧': 'Shift' };
const MODIFIERS = new Set(['Control', 'Meta', 'Alt', 'Shift']);

/**
 * 画面のキーの表示（「⌘ Enter」「Esc」「n」）を、aria-keyshortcuts の値（「Meta+Enter」「Escape」「n」）にする。
 * 「g i」のように続けて押すキーは aria-keyshortcuts で表せないので null（ショートカットの一覧「?」で伝える）。
 * @param {string} text
 * @returns {string|null}
 */
export function shortcutKeys(text) {
  const names = String(text || '').trim().split(/\s+/).filter(Boolean).map((k) => KEY_NAMES[k] || k);
  const keys = names.filter((k) => !MODIFIERS.has(k));
  return keys.length === 1 && names[names.length - 1] === keys[0] ? names.join('+') : null;
}

/**
 * ボタン。variant は primary / secondary / danger / ghost。kbd でショートカットを添える。
 * 例: button({ label: '保存', variant: 'primary', kbd: 'Ctrl Enter', onClick: save })
 */
export function button({ label, variant = 'secondary', iconName, kbd: keys, onClick, type = 'button', small = false, className, ...rest }) {
  return h(
    'button',
    {
      type,
      class: ['btn', variant !== 'secondary' && `btn--${variant}`, small && 'btn--sm', className],
      onclick: onClick,
      'aria-keyshortcuts': keys ? shortcutKeys(keys) : null,
      ...rest,
    },
    iconName ? icon(iconName, { size: small ? 16 : 18 }) : null,
    h('span', null, label),
    keys ? kbd(keys) : null,
  );
}

/** アイコンだけのボタン。label は読み上げ用（必須）。 */
export function iconButton({ iconName, label, onClick, className, ...rest }) {
  return h('button', { type: 'button', class: ['icon-btn', className], 'aria-label': label, title: label, onclick: onClick, ...rest }, icon(iconName));
}
