// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fold, filterItems, exactMatches, matchRank } from '../js/kana.js';

const ITEMS = [
  { value: 'exp_comm', label: '通信費', reading: 'つうしんひ' },
  { value: 'exp_supplies', label: '消耗品費', reading: 'しょうもうひんひ' },
  { value: 'exp_travel', label: '旅費交通費', reading: 'りょひこうつうひ' },
  { value: 'exp_fee', label: '支払手数料', reading: 'しはらいてすうりょう' },
  { value: 'sales', label: '売上高', reading: 'うりあげだか' },
];

test('AC-05.1 カタカナ・半角カナ・ひらがなを同じに畳む', () => {
  assert.equal(fold('ツウシン'), 'つうしん');
  assert.equal(fold('ﾂｳｼﾝ'), 'つうしん');
  assert.equal(fold('つうしん'), 'つうしん');
  assert.equal(fold('ＡＢＣ ｄｅｆ'), 'abcdef');
  assert.equal(fold('Exp_Comm'), 'exp_comm');
});

test('AC-05.1 名前・読み・コードの部分一致で絞る', () => {
  assert.deepEqual(filterItems(ITEMS, '通信').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(filterItems(ITEMS, 'つうしん').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(filterItems(ITEMS, 'ツウシン').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(filterItems(ITEMS, 'exp_f').map((i) => i.value), ['exp_fee']);
  assert.deepEqual(filterItems(ITEMS, 'まったく無い').length, 0);
});

test('AC-05.1 前方一致を上に、同じ順位は元の並びを保つ', () => {
  // 「こうつう」は旅費交通費の読みの途中にだけある。「ひ」は全部の読みにあり、前方一致は無い。
  assert.deepEqual(filterItems(ITEMS, 'こうつう').map((i) => i.value), ['exp_travel']);
  const hi = filterItems(ITEMS, 'し').map((i) => i.value);
  // 「し」で始まるのは消耗品費と支払手数料（前方一致）。通信費は読みの途中に「し」がある（部分一致）。
  assert.deepEqual(hi.slice(0, 2), ['exp_supplies', 'exp_fee']);
  assert.ok(hi.indexOf('exp_comm') > 1);
  assert.equal(matchRank(fold('つう'), ITEMS[0]), 0);
  assert.equal(matchRank(fold('しん'), ITEMS[0]), 1);
});

test('空の問い合わせは、すべての候補を元の順で返す', () => {
  assert.deepEqual(filterItems(ITEMS, '').map((i) => i.value), ITEMS.map((i) => i.value));
});

test('ちょうど一致する候補（欄を離れたときの自動選択）', () => {
  assert.deepEqual(exactMatches(ITEMS, '通信費').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(exactMatches(ITEMS, 'つうしんひ').map((i) => i.value), ['exp_comm']);
  assert.deepEqual(exactMatches(ITEMS, '通信').length, 0);
});
