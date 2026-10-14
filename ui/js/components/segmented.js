// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// いくつかから1つを選ぶボタンの並び（取引の種別など）。中身は標準のラジオボタンなので、
// 矢印での移動と読み上げはブラウザに任せる。key を持つ選択肢は、その数字キーでも選べる（keyboard.md の 1 2 3）。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';

/**
 * 例: createSegmented({ legend: '種別', options: [{ value: 'expense', label: '支出', key: '1' }, ...], value: 'expense', onChange })
 * @param {{ legend: string, options: Array<{ value: string, label: string, key?: string }>, value: string,
 *           onChange?: (value: string) => void, name?: string, className?: string }} opts
 */
export function createSegmented(opts) {
  const name = opts.name || uid('seg');
  const inputs = [];
  const labels = opts.options.map((o) => {
    const input = h('input', { type: 'radio', name, value: o.value, checked: o.value === opts.value });
    input.addEventListener('change', () => {
      if (input.checked && opts.onChange) opts.onChange(o.value);
    });
    inputs.push(input);
    return h(
      'label',
      { class: 'seg-option' },
      input,
      icon('check', { size: 16, className: 'seg-check' }),
      h('span', null, o.label),
      o.key ? h('kbd', { 'aria-hidden': 'true' }, o.key) : null,
    );
  });
  const el = h(
    'fieldset',
    { class: ['seg', opts.className] },
    h('legend', { class: 'field-label' }, opts.legend),
    h('div', { class: 'seg-options' }, labels),
  );

  function setValue(value, notify = false) {
    const input = inputs.find((i) => i.value === value);
    if (!input || input.checked) return;
    input.checked = true;
    if (notify && opts.onChange) opts.onChange(value);
  }

  // 選択肢の上で数字キーを押したら、その選択肢にする（フォーカスも移す）。
  el.addEventListener('keydown', (e) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const o = opts.options.find((x) => x.key && x.key === e.key);
    if (!o) return;
    e.preventDefault();
    setValue(o.value, true);
    const input = inputs.find((i) => i.value === o.value);
    if (input) input.focus();
  });

  return {
    el,
    get value() {
      const c = inputs.find((i) => i.checked);
      return c ? c.value : null;
    },
    setValue: (value, { notify = false } = {}) => setValue(value, notify),
    /** 選んである選択肢にフォーカスを移す。 */
    focus() {
      (inputs.find((i) => i.checked) || inputs[0]).focus();
    },
  };
}
