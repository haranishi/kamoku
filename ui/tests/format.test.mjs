// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  formatYen,
  formatSigned,
  formatDate,
  formatDateShort,
  formatMonth,
  formatPct,
  formatCompactYen,
  describeDiff,
  shortHash,
} from '../js/format.js';

test('AC-11.5 金額は桁区切り、負は半角ハイフン', () => {
  assert.equal(formatYen(11000), '11,000');
  assert.equal(formatYen(0), '0');
  assert.equal(formatYen(999), '999');
  assert.equal(formatYen(1000), '1,000');
  assert.equal(formatYen(-1200), '-1,200');
  assert.equal(formatYen(9999999999999), '9,999,999,999,999');
  assert.equal(formatYen(Number.NaN), '');
  assert.equal(formatYen(undefined), '');
});

test('符号つきの金額（入金を正、出金を負）', () => {
  assert.equal(formatSigned(1650), '+1,650');
  assert.equal(formatSigned(-1650), '-1,650');
  assert.equal(formatSigned(0), '0');
});

test('日付と曜日', () => {
  assert.equal(formatDate('2026-10-05'), '2026-10-05 (月)');
  assert.equal(formatDate('2026-10-14'), '2026-10-14 (水)');
  assert.equal(formatDate('2028-02-29'), '2028-02-29 (火)');
  assert.equal(formatDateShort('2026-10-05'), '10/5');
  assert.equal(formatMonth('2026-10'), '2026年10月');
  assert.equal(formatPct(70), '70%');
});

test('グラフの目盛りの短い金額', () => {
  assert.equal(formatCompactYen(0), '0');
  assert.equal(formatCompactYen(5000), '5,000');
  assert.equal(formatCompactYen(50000), '5万');
  assert.equal(formatCompactYen(15000), '1.5万');
  assert.equal(formatCompactYen(1200000), '120万');
  assert.equal(formatCompactYen(-300000), '-30万');
  assert.equal(formatCompactYen(150000000), '1.5億');
});

test('AC-07.4 前の月との差は、向きと文で返す（色に頼らない）', () => {
  assert.deepEqual(describeDiff(250800, 240000), { dir: 'up', text: '前月より 10,800円 多い' });
  assert.deepEqual(describeDiff(96400, 101000), { dir: 'down', text: '前月より 4,600円 少ない' });
  assert.deepEqual(describeDiff(5, 5), { dir: 'same', text: '前月と同じ' });
  assert.deepEqual(describeDiff(-1000, 2000), { dir: 'down', text: '前月より 3,000円 少ない' });
});

test('ハッシュの先頭8桁', () => {
  assert.equal(shortHash('ab12cd34ef567890'), 'ab12cd34');
  assert.equal(shortHash(null), '');
});
