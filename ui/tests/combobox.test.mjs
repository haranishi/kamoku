// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { filterOptions, pickOnBlur, sameName, optionsWithNew, listPlacement } from '../js/components/combobox.js';

test('SC-02 候補の一覧: 下の余白が6件ぶん（288px）に足りず、上のほうが広ければ上に開き、欄を隠さない', () => {
  // 幅390で、画面の下寄りの欄（上端599px）。前は下に開いて、下端までの約5件しか見えなかった。
  const low = listPlacement({ top: 599, bottom: 643, left: 33, width: 324 }, 390, 844);
  assert.equal(low.above, true);
  assert.equal(low.maxHeight, 288, '6件ぶん見える');
  assert.equal(low.bottom, 844 - 599 + 4, '一覧の下の端は、欄の上の端より上');
  assert.equal(low.top, null);
  // 下に6件ぶんの余白があれば、今までどおり下。
  const high = listPlacement({ top: 200, bottom: 244, left: 33, width: 324 }, 390, 844);
  assert.equal(high.above, false);
  assert.equal(high.top, 248);
  assert.equal(high.maxHeight, 288);
  // 下の余白が足りなくても、上のほうが狭ければ下（低い画面）。
  const short = listPlacement({ top: 120, bottom: 164, left: 33, width: 324 }, 640, 450);
  assert.equal(short.above, false);
  assert.equal(short.maxHeight, 450 - 164 - 8);
  // 上に開くときも、高さは上の余白まで。
  const both = listPlacement({ top: 250, bottom: 294, left: 33, width: 324 }, 640, 450);
  assert.equal(both.above, true);
  assert.equal(both.maxHeight, 242);
  // 幅は欄の幅（240〜420px、画面の幅から16pxを引いた幅まで）、左の端は画面の中。
  assert.equal(listPlacement({ top: 0, bottom: 44, left: 300, width: 200 }, 390, 844).left, 390 - 240 - 8);
});

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

const PARTNERS = [
  { value: 'サンプル通信社', label: 'サンプル通信社', reading: 'さんぷるつうしんしゃ' },
  { value: 'サンプル通信', label: 'サンプル通信', reading: 'さんぷるつうしん' },
];
const newLabel = (t) => `「${t}」を新しい取引先として足す`;

test('SC-02 取引先: 打った名前がどの候補とも同じでなければ、末尾に「新しい取引先として足す」の行を足す', () => {
  const list = optionsWithNew(PARTNERS, '通信', newLabel);
  assert.deepEqual(list.map((i) => i.value), ['サンプル通信社', 'サンプル通信', '通信']);
  assert.equal(list.at(-1).isNew, true);
  assert.equal(list.at(-1).optionLabel, '「通信」を新しい取引先として足す');
  assert.ok(!list.slice(0, -1).some((i) => i.isNew));
  // 候補が1つも無くても、その1行だけは出す。空の文字では出さない。
  assert.deepEqual(optionsWithNew(PARTNERS, '山田商店', newLabel).map((i) => [i.value, Boolean(i.isNew)]), [['山田商店', true]]);
  assert.equal(optionsWithNew(PARTNERS, '  ', newLabel).some((i) => i.isNew), false);
});

test('SC-02 取引先: 名前がちょうど同じ候補があれば、それを先頭にし、新しい名前の行は出さない（全角と半角、かなの違いは同じ）', () => {
  assert.deepEqual(optionsWithNew(PARTNERS, 'サンプル通信', newLabel).map((i) => i.value), ['サンプル通信', 'サンプル通信社']);
  assert.equal(optionsWithNew(PARTNERS, 'ｻﾝﾌﾟﾙ通信', newLabel).some((i) => i.isNew), false);
  // 読みがちょうど同じでも、名前が違えば新しい名前の行は出す（選ぶかどうかは本人が決める）。
  assert.equal(optionsWithNew(PARTNERS, 'さんぷるつうしん', newLabel).at(-1).isNew, true);
  assert.deepEqual(sameName(PARTNERS, 'サンプル通信').map((i) => i.value), ['サンプル通信']);
  assert.deepEqual(sameName(PARTNERS, ''), []);
});
