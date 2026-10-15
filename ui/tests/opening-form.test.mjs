// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readBalance } from '../js/screens/opening-form.js';

test('SC-10 期首残高の欄: 空と0は0円、金額の式も読む。答えが0円の式もよい', () => {
  for (const [text, value] of [['', 0], ['0', 0], ['000', 0], ['1,650', 1650], ['１０００', 1000], ['(1200+300)*2', 3000], ['5-5', 0], ['0*7', 0], ['-0', 0]]) {
    assert.deepEqual(readBalance(text), { ok: true, value }, text);
  }
});

test('SC-10 期首残高の欄: マイナスは「0円は可」と添えて断る（すぐ下の「0のままで進めてかまいません」と食い違わないように）', () => {
  for (const text of ['-500', 'ー500', '−1,200', '100-200']) {
    assert.deepEqual(readBalance(text), { ok: false, error: 'マイナスの金額は入れられません（0円は可）。' }, text);
  }
  // 読めない文字は、金額の欄の決まりの文のまま。
  const r = readBalance('abc');
  assert.equal(r.ok, false);
  assert.match(r.error, /使えない文字/);
});
