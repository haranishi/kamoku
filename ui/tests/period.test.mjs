// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readPeriod, periodApi, periodQuery, periodLabel, shiftPeriod, carryPeriod, checkRange } from '../js/period.js';

const today = '2026-10-14';

test('AC-08.7 期間の既定は今年。年・月・自由な期間をアドレスから読む', () => {
  assert.deepEqual(periodApi(readPeriod({}, { today })), { from: '2026-01-01', to: '2026-12-31' });
  assert.deepEqual(periodApi(readPeriod({ year: '2025' }, { today })), { from: '2025-01-01', to: '2025-12-31' });
  const m = readPeriod({ month: '2026-02' }, { today });
  assert.equal(m.mode, 'month');
  assert.deepEqual(periodApi(m), { from: '2026-02-01', to: '2026-02-28' });
  const r = readPeriod({ from: '2026-04-01', to: '2026-06-30' }, { today });
  assert.equal(r.mode, 'range');
  assert.deepEqual(periodQuery(r), { from: '2026-04-01', to: '2026-06-30' });
});

test('AC-08.7 貸借対照表は日付を1つ。既定は今日', () => {
  assert.deepEqual(periodApi(readPeriod({}, { today, dateMode: true })), { as_of: '2026-10-14' });
  assert.deepEqual(periodApi(readPeriod({ as_of: '2025-12-31' }, { today, dateMode: true })), { as_of: '2025-12-31' });
  // 実在しない日は、今日に戻す。
  assert.deepEqual(periodApi(readPeriod({ as_of: '2026-02-30' }, { today, dateMode: true })), { as_of: '2026-10-14' });
});

test('AC-08.8 年をまたぐ自由な期間は読まない（理由の文を返す）', () => {
  assert.match(checkRange('2025-12-01', '2026-01-31'), /同じ年の中/);
  assert.match(checkRange('2026-06-01', '2026-04-01'), /後になっています/);
  assert.equal(checkRange('2026-04-01', '2026-06-30'), '');
  // アドレスにまたぐ期間が書かれていても、今年に戻す（サーバーへ送らない）。
  const p = readPeriod({ from: '2025-12-01', to: '2026-01-31' }, { today });
  assert.equal(p.mode, 'year');
  assert.equal(p.year, 2026);
});

test('月次推移は年だけ（月の指定があっても、その年にする）', () => {
  const p = readPeriod({ month: '2025-03' }, { today, yearOnly: true });
  assert.equal(p.mode, 'year');
  assert.equal(p.year, 2026);
  assert.equal(readPeriod({ year: '2025' }, { today, yearOnly: true }).year, 2025);
});

test('前と次へずらす: 年は1年、月は1か月（年をまたいでもよい）、日付は前後の月の末日', () => {
  assert.equal(shiftPeriod(readPeriod({ year: '2026' }, { today }), -1).year, 2025);
  const jan = shiftPeriod(readPeriod({ month: '2026-01' }, { today }), -1);
  assert.deepEqual(periodApi(jan), { from: '2025-12-01', to: '2025-12-31' });
  assert.deepEqual(periodApi(shiftPeriod(readPeriod({ as_of: '2026-03-15' }, { today, dateMode: true }), -1)), { as_of: '2026-02-28' });
});

test('タブを移るときの引き継ぎ: 今年なら今日、過去の年なら年末の貸借対照表', () => {
  assert.equal(carryPeriod(readPeriod({ year: '2026' }, { today }), { today, dateMode: true }).asOf, '2026-10-14');
  assert.equal(carryPeriod(readPeriod({ year: '2025' }, { today }), { today, dateMode: true }).asOf, '2025-12-31');
  assert.equal(carryPeriod(readPeriod({ month: '2026-03' }, { today }), { today, dateMode: true }).asOf, '2026-03-31');
  const back = carryPeriod(readPeriod({ as_of: '2025-06-30' }, { today, dateMode: true }), { today });
  assert.deepEqual(periodQuery(back), { year: '2025' });
});

test('期間の見出し', () => {
  assert.equal(periodLabel(readPeriod({}, { today })), '2026年');
  assert.equal(periodLabel(readPeriod({ month: '2026-10' }, { today })), '2026年10月');
  assert.equal(periodLabel(readPeriod({ from: '2026-04-01', to: '2026-06-30' }, { today })), '2026-04-01 〜 2026-06-30');
  assert.equal(periodLabel(readPeriod({ as_of: '2026-10-14' }, { today, dateMode: true })), '2026-10-14 時点');
});
