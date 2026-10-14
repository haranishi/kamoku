// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 金額と「円」、値の無いときの表し方。全画面で同じ書き方にするため、ここだけで作る。

import { h } from '../dom.js';
import { formatYen } from '../format.js';

/**
 * 金額と「円」を、折り返しでも離れない1つのかたまりにして返す。
 * 「円」は小さく、数字との間に空白を入れない（見た目は components.css の .money）。
 * @param {number} n 整数の円
 */
export function money(n) {
  return h('span', { class: 'money' }, h('span', { class: 'num' }, formatYen(n)), h('span', { class: 'unit' }, '円'));
}

/** 値が無いときの表し方。見た目は「—」、読み上げは「なし」。 */
export function emptyValue() {
  return h('span', { class: 'empty-value' }, h('span', { 'aria-hidden': 'true' }, '—'), h('span', { class: 'visually-hidden' }, 'なし'));
}
