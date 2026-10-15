// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 月次の推移のグラフ（js/components/chart.js）の、描く前の計算。
// 記録のない月（収益も経費も0）は、点も線も描かず線を切る。0へ急に落ちたように見せないため（SC-01、SC-06）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isEmptyMonth, profitSegments, axisRange, summarize, lossLabels, labelWidth } from '../js/components/chart.js';

test('SC-06 赤字の月に添える額: 赤字の月だけ。横に重なる月は次の段へずらし、段が足りない月だけ省く。左右の端からはみ出さない', () => {
  const pts = [
    { index: 0, profit: 120000 },
    { index: 3, profit: -25175 },
    { index: 4, profit: -8000 },
    { index: 7, profit: -300000 },
  ];
  // 幅1440: 月の間が広いので、赤字の3か月とも同じ段（0）に置ける。黒字の月には置かない。
  const wide = lossLabels(pts, (i) => 52 + 80 * i + 40, { left: 52, right: 1012 });
  assert.deepEqual(wide.map((l) => [l.index, l.text, l.row]), [[3, '-2.5万', 0], [4, '-8,000', 0], [7, '-30万', 0]]);
  // 幅390: 月の間が約22px。隣り合う4月と5月は重なるので、5月を次の段（1）へずらす。額は省かない（点の塗りに頼らない）。
  const narrow = lossLabels(pts, (i) => 52 + 22 * i + 11, { left: 52, right: 316 });
  assert.deepEqual(narrow.map((l) => [l.index, l.row]), [[3, 0], [4, 1], [7, 0]]);
  // 段は2つまで。月の間が14pxしかなく、3つ続けて重なるときは、3つ目を省く。
  const tight = lossLabels([{ index: 3, profit: -9000 }, { index: 4, profit: -9000 }, { index: 5, profit: -9000 }], (i) => 52 + 14 * i, { left: 52, right: 316 });
  assert.deepEqual(tight.map((l) => [l.index, l.row]), [[3, 0], [4, 1]]);
  // 端の月は、文字が左右の端からはみ出さないよう、内側へ寄せる。
  const edge = lossLabels([{ index: 0, profit: -1200000 }], () => 60, { left: 52, right: 316 });
  assert.ok(edge[0].x - labelWidth(edge[0].text) / 2 >= 52, JSON.stringify(edge));
  assert.deepEqual(lossLabels([{ index: 1, profit: 0 }, { index: 2, profit: 5 }], (i) => i * 40, { left: 0, right: 400 }), []);
  // 「万」「億」は数字より幅を取る。
  assert.ok(labelWidth('-2.5万') > labelWidth('-2.55'));
});

const m = (month, revenue, expense) => ({ month, revenue, expense, profit: revenue - expense });

test('SC-06 記録のない月は、収益も経費も0の月だけ（片方でも0でなければ記録がある）', () => {
  assert.equal(isEmptyMonth(m('2026-10', 0, 0)), true);
  assert.equal(isEmptyMonth(m('2026-10', 0, 5000)), false);
  assert.equal(isEmptyMonth(m('2026-10', 5000, 0)), false);
  // 返品などで経費が負の月も、記録がある。
  assert.equal(isEmptyMonth(m('2026-10', 0, -300)), false);
});

test('SC-06 利益の線は、記録のない月で切り、今月より後の月は入れない', () => {
  const series = [
    m('2026-01', 100, 40),
    m('2026-02', 0, 0), // 記録のない月（ここで線を切る）
    m('2026-03', 200, 250),
    m('2026-04', 300, 100),
    m('2026-05', 0, 0),
    m('2026-06', 0, 0),
    m('2026-07', 50, 10),
    m('2026-08', 70, 10), // 今月より後（描かない）
  ];
  const segs = profitSegments(series, '2026-07');
  assert.deepEqual(
    segs.map((s) => s.map((p) => [p.index, p.profit])),
    [[[0, 60]], [[2, -50], [3, 200]], [[6, 40]]],
  );
  // until が無ければ、記録のある月はすべて入る。
  assert.deepEqual(profitSegments(series).map((s) => s.map((p) => p.month)).at(-1), ['2026-07', '2026-08']);
  // 記録が1つも無ければ、線は無い（空の区間を作らない）。
  assert.deepEqual(profitSegments([m('2026-01', 0, 0), m('2026-02', 0, 0)], '2026-12'), []);
});

test('SC-06 縦軸: 0より下は、小さな赤字のために1目盛りぶん伸ばさない（目盛りの5分の1の刻み）', () => {
  const r = axisRange([0, 1005400, 330000, -25175, 882682]);
  assert.equal(r.step, 500000);
  assert.equal(r.hi, 1500000);
  // -25,175 のために -500,000 まで伸ばさず、-100,000（目盛りの5分の1）で止める。
  assert.equal(r.lo, -100000);
  assert.ok(r.lo <= -25175 && r.lo > -r.step);
  // 0より下の値が無ければ、0から始める。
  assert.deepEqual(axisRange([0, 120, 80]), { lo: 0, hi: 150, step: 50 });
  // 全部0でも、軸の幅を作る（0で割らない）。
  const z = axisRange([0, 0, 0]);
  assert.ok(z.hi > z.lo);
  assert.equal(z.lo, 0);
  // 大きな赤字は、その値まで下げる（切らない）。
  const deep = axisRange([0, 100000, -900000]);
  assert.ok(deep.lo <= -900000);
  assert.ok(deep.hi >= 100000);
});

test('SC-06 読み上げの要約は、記録のない月と今月より後の月を数えない', () => {
  const series = [m('2026-01', 100, 40), m('2026-02', 0, 0), m('2026-03', 200, 250), m('2026-04', 900, 0)];
  const text = summarize(series, '2026-03');
  assert.match(text, /^2026年1月〜3月の推移。/);
  assert.match(text, /売上の合計 300円/);
  assert.match(text, /最も少ないのは3月の-50円（赤字）/);
  assert.equal(summarize([m('2026-01', 0, 0)], '2026-12'), '表示できる月がありません。');
});
