// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  parseAmountExpr,
  parseDateInput,
  addDays,
  monthRange,
  shiftMonth,
  normalizeWidth,
} from '../js/input-parse.js';

const ok = (text) => {
  const r = parseAmountExpr(text);
  assert.equal(r.ok, true, `${text}: ${r.error}`);
  return r;
};
const ng = (text) => {
  const r = parseAmountExpr(text);
  assert.equal(r.ok, false, `${text} は断るはず`);
  assert.ok(r.error.length > 0);
  return r;
};

test('AC-02.3 金額: 「1,650」「１６５０」「1000+650」を受け付ける', () => {
  assert.equal(ok('1,650').value, 1650);
  assert.equal(ok('１６５０').value, 1650);
  assert.equal(ok('1000+650').value, 1650);
  assert.equal(ok('１０００＋６５０').value, 1650);
});

test('AC-02.3 金額: 円記号・円・空白・全角カンマを除く', () => {
  assert.equal(ok('¥1,650').value, 1650);
  assert.equal(ok('￥１，６５０円').value, 1650);
  assert.equal(ok(' 1 650 ').value, 1650);
  assert.equal(ok('\\1650').value, 1650);
});

test('AC-02.3 金額: 桁区切りの位置が違う（1,2,3 など）ときは断る。正しい区切りと全角は通す', () => {
  for (const bad of ['1,2,3', '1,0000', '10,00', ',100', '100,', '1,000,00']) assert.match(ng(bad).error, /桁区切りの位置が違います。/, bad);
  assert.equal(ok('1,234').value, 1234);
  assert.equal(ok('12,345,678').value, 12345678);
  assert.equal(ok('１，２３４').value, 1234);
  assert.equal(ok('1,000+2,500').value, 3500);
  assert.equal(ok('(1,200)*2').value, 2400);
});

test('AC-02.3 金額: 四則と丸括弧、演算子の優先順位', () => {
  assert.equal(ok('(1200+300)*2').value, 3000);
  assert.equal(ok('1200+300*2').value, 1800);
  assert.equal(ok('10000-2000-3000').value, 5000);
  assert.equal(ok('-500+1000').value, 500);
  assert.equal(ok('2×3000').value, 6000);
  assert.equal(ok('9000÷3').value, 3000);
  assert.equal(ok('1200+300').isExpr, true);
  assert.equal(ok('1500').isExpr, false);
});

test('AC-02.3 金額: 日本語入力の長音「ー」や全角の「－」を引き算として読む', () => {
  assert.equal(ok('1000ー650').value, 350);
  assert.equal(ok('1000－650').value, 350);
  assert.equal(ok('1000−650').value, 350);
});

test('AC-02.3 金額: 割り算は切り捨てで、切り捨てたことを知らせる', () => {
  const r = ok('100/3');
  assert.equal(r.value, 33);
  assert.equal(r.rounded, true);
  assert.equal(ok('99/3').rounded, false);
});

test('AC-02.3 金額: 途中の値が大きくても整数のまま正しく計算する（浮動小数点なら0になる）', () => {
  assert.equal(ok('9007199254740993-9007199254740992').value, 1);
  assert.equal(ok('(9999999999999*9999+1)/9999').value, 9999999999999);
});

test('AC-02.3 金額: 1円未満と上限超えを断る', () => {
  ng('0');
  ng('1-1');
  ng('100-200');
  assert.equal(ok('9999999999999').value, 9999999999999);
  assert.match(ng('10000000000000').error, /9,999,999,999,999/);
});

test('AC-02.3 金額: 形の誤りは理由を返す（evalを使わない）', () => {
  assert.equal(ng('').empty, true);
  assert.match(ng('1.5').error, /小数/);
  assert.match(ng('abc').error, /使えない文字/);
  assert.match(ng('alert(1)').error, /使えない文字/);
  assert.match(ng('1+').error, /式の形/);
  assert.match(ng('((1)').error, /かっこ/);
  assert.match(ng('10/0').error, /0では割れません/);
  assert.match(ng('1'.repeat(101)).error, /長すぎ/);
  ng(`${'('.repeat(30)}1${')'.repeat(30)}`);
  ng('-'.repeat(40) + '1');
});

const T = { today: '2026-10-14' };
const date = (text, opts = T) => {
  const r = parseDateInput(text, opts);
  assert.equal(r.ok, true, `${text}: ${r.error}`);
  return r.value;
};

test('AC-02.4 日付: 「1005」「10/5」「10-5」「20261005」などを受け付ける', () => {
  assert.equal(date('1005'), '2026-10-05');
  assert.equal(date('10/5'), '2026-10-05');
  assert.equal(date('10-5'), '2026-10-05');
  assert.equal(date('2026/10/5'), '2026-10-05');
  assert.equal(date('2026-10-05'), '2026-10-05');
  assert.equal(date('20261005'), '2026-10-05');
  assert.equal(date('１００５'), '2026-10-05');
  assert.equal(date('１０／５'), '2026-10-05');
  assert.equal(date('2026年10月5日'), '2026-10-05');
});

test('AC-02.4 日付: 「t」は今日、「y」は昨日', () => {
  assert.equal(date('t'), '2026-10-14');
  assert.equal(date('Ｔ'), '2026-10-14');
  assert.equal(date('y'), '2026-10-13');
  assert.equal(date('y', { today: '2026-01-01' }), '2025-12-31');
});

test('AC-02.4 日付: 年を省いたときは year の指定、なければ今日の年', () => {
  assert.equal(date('1231', { today: '2027-01-02', year: 2026 }), '2026-12-31');
  assert.equal(date('1231', { today: '2027-01-02' }), '2027-12-31');
});

test('AC-02.4 日付: 実在しない日と読めない形を断る', () => {
  for (const bad of ['0230', '2026-02-29', '13/1', '0/5', '1032', '105', 'abc', '2026-1-1-1']) {
    const r = parseDateInput(bad, T);
    assert.equal(r.ok, false, `${bad} は断るはず`);
  }
  assert.equal(parseDateInput('', T).empty, true);
  assert.equal(date('2028-02-29'), '2028-02-29');
});

test('AC-02.4 日付: 1日ずつ動かす（月末・年末・うるう年）', () => {
  assert.equal(addDays('2026-10-31', 1), '2026-11-01');
  assert.equal(addDays('2026-12-31', 1), '2027-01-01');
  assert.equal(addDays('2026-01-01', -1), '2025-12-31');
  assert.equal(addDays('2028-02-28', 1), '2028-02-29');
  assert.equal(addDays('2026-02-28', 1), '2026-03-01');
});

test('月の範囲と月のずらし', () => {
  assert.deepEqual(monthRange('2026-02'), { from: '2026-02-01', to: '2026-02-28' });
  assert.deepEqual(monthRange('2028-02'), { from: '2028-02-01', to: '2028-02-29' });
  assert.equal(shiftMonth('2026-01', -1), '2025-12');
  assert.equal(shiftMonth('2026-12', 1), '2027-01');
});

test('幅をそろえる', () => {
  assert.equal(normalizeWidth('１２３ＡＢＣ（）'), '123ABC()');
});
