// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { filterOptions, pickOnBlur } from '../js/components/combobox.js';

// 種別に合う分類を上に並べた候補（支出なら費用が先）。
const ACCOUNTS = [
  { value: 'exp_comm', label: '通信費', reading: 'つうしんひ', hint: '費用' },
  { value: 'exp_travel', label: '旅費交通費', reading: 'りょひこうつうひ', hint: '費用' },
  { value: 'exp_supplies', label: '消耗品費', reading: 'しょうもうひんひ', hint: '費用' },
  { value: 'exp_utility', label: '水道光熱費', reading: 'すいどうこうねつひ', hint: '費用' },
  { value: 'sales', label: '売上高', reading: 'うりあげだか', hint: '収益' },
  { value: 'w03', label: 'サンプルカード', reading: 'さんぷるかーど', hint: 'カード' },
];

test('AC-05.1 ひらがなの読みで絞れる（変換前の文字でも絞れる）', () => {
  assert.deepEqual(filterOptions(ACCOUNTS, 'つうしん').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(filterOptions(ACCOUNTS, 'こうつう').map((i) => i.value), ['exp_travel']);
});

test('AC-05.1 漢字の名前、カタカナ、コードでも絞れる', () => {
  assert.deepEqual(filterOptions(ACCOUNTS, '光熱').map((i) => i.value), ['exp_utility']);
  assert.deepEqual(filterOptions(ACCOUNTS, 'カード').map((i) => i.value), ['w03']);
  assert.deepEqual(filterOptions(ACCOUNTS, 'exp_s').map((i) => i.value), ['exp_supplies']);
});

test('AC-05.1 前方一致が上、同じ順位なら呼び出し側の並び（種別に合う分類が上）を保つ', () => {
  // 「ひ」で始まる候補は無く、読みの途中の「ひ」は費用の4つ。並びは元のまま。
  assert.deepEqual(filterOptions(ACCOUNTS, 'ひ').map((i) => i.value), ['exp_comm', 'exp_travel', 'exp_supplies', 'exp_utility']);
  // 「う」で始まるのは売上高（前方一致）。通信費（つ「う」しんひ）より上に来る。
  const u = filterOptions(ACCOUNTS, 'う').map((i) => i.value);
  assert.equal(u[0], 'sales');
  assert.ok(u.includes('exp_comm'));
});

test('候補の数の上限', () => {
  const many = Array.from({ length: 300 }, (_, i) => ({ value: `x${i}`, label: `科目${i}` }));
  assert.equal(filterOptions(many, '').length, 100);
  assert.equal(filterOptions(many, '', 20).length, 20);
});

test('欄を離れたとき、名前か読みがちょうど1つに一致すれば選ぶ', () => {
  assert.equal(pickOnBlur(ACCOUNTS, '通信費').value, 'exp_comm');
  assert.equal(pickOnBlur(ACCOUNTS, 'ツウシンヒ').value, 'exp_comm');
  assert.equal(pickOnBlur(ACCOUNTS, '通信'), null);
  const dup = [...ACCOUNTS, { value: 'x01', label: '通信費', reading: 'つうしんひ' }];
  assert.equal(pickOnBlur(dup, '通信費'), null, '2つに一致するときは選ばない');
});
