// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 入力欄とボタンの組み立て。見えるラベル、欄のすぐ下のエラー、aria-describedby の結び付けを、ここに集める（nfr.md）。

import { h, uid, addDescribedBy, removeDescribedBy } from '../dom.js';
import { icon } from '../icons.js';

/**
 * ラベル・入力・説明・エラーの一式を作る。
 * 例: const f = field({ label: '日付', control: h('input', { class: 'input' }) }); f.setError('日付を入れてください。');
 * @param {{ label: string, control: HTMLElement, id?: string, hint?: string|Node, required?: boolean,
 *           labelFor?: HTMLElement, className?: string, after?: Node }} opts
 *   labelFor: ラベルを結ぶ要素（候補欄など、control の中の input を指すとき）
 * @returns {{ el: HTMLElement, control: HTMLElement, hint: HTMLElement|null, setError: (msg?: string) => void,
 *             setHint: (text: string) => void, setLabel: (text: string) => void }}
 */
export function field({ label, control, id, hint, required = false, labelFor, className, after }) {
  const target = labelFor || control;
  const controlId = target.id || id || uid('f');
  target.id = controlId;
  const hintEl = hint ? h('p', { class: 'field-hint', id: `${controlId}-hint` }, hint) : null;
  const errorEl = h('p', { class: 'field-error', id: `${controlId}-err` });
  if (hintEl) addDescribedBy(target, hintEl.id);
  const labelText = h('span', null, label);
  const el = h(
    'div',
    { class: ['field', className] },
    h('label', { class: 'field-label', for: controlId }, labelText, required ? h('span', { class: 'req' }, '必須') : null),
    control,
    after || null,
    hintEl,
    errorEl,
  );
  return {
    el,
    control,
    hint: hintEl,
    setLabel(text) {
      labelText.textContent = text;
    },
    setError(message) {
      errorEl.replaceChildren();
      if (message) {
        errorEl.append(icon('alert', { size: 16 }), h('span', null, message));
        target.setAttribute('aria-invalid', 'true');
        addDescribedBy(target, errorEl.id);
      } else {
        target.removeAttribute('aria-invalid');
        removeDescribedBy(target, errorEl.id);
      }
    },
    setHint(text) {
      if (hintEl) hintEl.textContent = text;
    },
  };
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

/** キーの表示（「Ctrl」「Enter」など）。 */
export function kbd(text) {
  return h('kbd', null, text);
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
