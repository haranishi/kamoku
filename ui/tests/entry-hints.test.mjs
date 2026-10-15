// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parsePct, pctSourceNote, taxSourceNote, jpDate, dateConcern, lineMemo, lockedMessage, lockedDetailNote } from '../js/screens/entry-hints.js';

test('AC-03.4 事業の割合: 0〜100の整数だけを読む（150、-10、abc、小数、「5e1」は読まない）', () => {
  assert.equal(parsePct('70'), 70);
  assert.equal(parsePct('０'), 0);
  assert.equal(parsePct(' 100 '), 100);
  for (const bad of ['150', '-10', 'abc', '', '12.5', '5e1', '1000']) assert.equal(parsePct(bad), null, bad);
});

test('AC-03.1 割合の出どころ: 科目の既定が100%未満のときだけ出す（口座の科目には出さない）', () => {
  assert.equal(pctSourceNote({ kind: 'expense', default_pct: 30 }), 'この科目の既定の割合は、事業30%です。「勘定科目」の画面で変えられます。');
  assert.equal(pctSourceNote({ kind: 'expense', default_pct: 100 }), '');
  assert.equal(pctSourceNote({ kind: 'expense' }), '');
  assert.equal(pctSourceNote({ kind: 'asset', default_pct: 30, wallet: { type: 'card' } }), '');
  assert.equal(pctSourceNote(null), '');
});

test('AC-01.4 課税事業者で、既定が「対象外」の費用の科目を「対象外」のまま使うときだけ、一言を出す', () => {
  const travel = { kind: 'expense', default_tax: null };
  const note = 'この科目の既定は「対象外」です。課税の取引なら、税区分を選んでください。';
  assert.equal(taxSourceNote({ taxable: true, kind: 'expense', account: travel, taxCode: null }), note);
  assert.equal(taxSourceNote({ taxable: true, kind: 'expense', account: travel, taxCode: 'P10' }), '', '税区分を選んだら消す');
  assert.equal(taxSourceNote({ taxable: false, kind: 'expense', account: travel, taxCode: null }), '', '免税には出さない');
  assert.equal(taxSourceNote({ taxable: true, kind: 'income', account: { kind: 'revenue', default_tax: null }, taxCode: null }), '', '収益には出さない');
  assert.equal(taxSourceNote({ taxable: true, kind: 'expense', account: { kind: 'asset', default_tax: null }, taxCode: null }), '', '資産には出さない');
  assert.equal(taxSourceNote({ taxable: true, kind: 'expense', account: { kind: 'expense', default_tax: 'P10' }, taxCode: null }), '', '既定が対象外でない科目には出さない（文が事実と合わない）');
});

test('SC-02 日付の取り違え: 今日より31日を超えて先か、開業日より前なら、確認の文を返す', () => {
  const today = '2026-10-15';
  assert.equal(jpDate('2030-01-01'), '2030年1月1日');
  assert.equal(dateConcern('2026-11-15', { today, openedOn: '2026-01-01' }), '', 'ちょうど31日先は聞かない');
  assert.match(dateConcern('2026-11-16', { today, openedOn: '2026-01-01' }), /^2026年11月16日の取引として保存します。今日より31日を超えて先の日付です。年は合っていますか。$/);
  assert.match(dateConcern('2025-12-31', { today, openedOn: '2026-01-01' }), /開業日（2026年1月1日）より前の日付です。/);
  assert.equal(dateConcern('2026-01-01', { today, openedOn: '2026-01-01' }), '');
  assert.equal(dateConcern('2025-12-31', { today, openedOn: '' }), '', '開業日の無い帳簿では、前の日付を聞かない');
  assert.equal(dateConcern('', { today, openedOn: '2026-01-01' }), '');
});

test('AC-03.1 事業主貸の「家事按分 30%」は、私用の分の割合なので「私用の分 30%」と言い換える。ほかのメモはそのまま', () => {
  assert.equal(lineMemo({ account: 'owner_draw', memo: '家事按分 30%' }), '私用の分 30%');
  assert.equal(lineMemo({ account: 'owner_draw', memo: '家事按分 100%' }), '私用の分 100%');
  assert.equal(lineMemo({ account: 'exp_comm', memo: '家事按分 30%' }), '家事按分 30%');
  assert.equal(lineMemo({ account: 'owner_draw', memo: '立替の精算' }), '立替の精算');
  assert.equal(lineMemo({ account: 'w01', memo: '' }), '');
});

test('AC-06.3 締めた日の断り: 締めた日「以前」（その日を含む）だけ。締めていなければ何も出さない', () => {
  const locked = '2026-09-30';
  assert.equal(lockedMessage({ date: '2026-09-30', lockedUntil: locked }), '2026年9月30日まで締めてあるため、この日付には登録できません。');
  assert.equal(lockedMessage({ date: '2026-09-15', lockedUntil: locked }), '2026年9月30日まで締めてあるため、この日付には登録できません。');
  assert.equal(lockedMessage({ date: '2026-10-01', lockedUntil: locked }), '');
  for (const none of [null, '', undefined, '2026/09/30']) assert.equal(lockedMessage({ date: '2026-09-15', lockedUntil: none }), '', String(none));
  assert.equal(lockedMessage({ date: null, lockedUntil: locked }), '');
});

test('AC-06.3 保存で断られたときは直し方まで書く。訂正は「訂正できません」、元の日付が締めた日以前なら、その仕訳は訂正できないと書く', () => {
  const locked = '2026-09-30';
  assert.equal(
    lockedMessage({ date: '2026-09-15', lockedUntil: locked, onSave: true }),
    '2026年9月30日まで締めてあるため、この日付には登録できません。日付を直すか、設定の「締め」で締めた日を見直してください。',
  );
  assert.equal(lockedMessage({ date: '2026-09-15', lockedUntil: locked, correcting: true, originalDate: '2026-10-02' }), '2026年9月30日まで締めてあるため、この日付には訂正できません。');
  // 元の日付が締めた日以前なら、変更先の日付に関係なく訂正できない（サーバーの決まり）。
  assert.equal(lockedMessage({ date: '2026-10-05', lockedUntil: locked, correcting: true, originalDate: '2026-09-10' }), '2026年9月30日まで締めてあるため、2026年9月10日の仕訳は訂正できません。');
  assert.equal(
    lockedMessage({ date: '2026-10-05', lockedUntil: locked, correcting: true, originalDate: '2026-09-10', onSave: true }),
    '2026年9月30日まで締めてあるため、2026年9月10日の仕訳は訂正できません。締めた日は、設定の「締め」で見直せます。',
  );
});

test('AC-06.3 仕訳の詳細: 締めた日以前の仕訳には、訂正も取消もできない文を出す（その日を含む）。締めていない・後の日付なら出さない', () => {
  assert.equal(lockedDetailNote('2026-10-02', '2026-10-10'), '2026年10月10日まで締めてあるため、この仕訳は訂正も取消もできません。締めた日は、設定の「締め」で見直せます。');
  assert.notEqual(lockedDetailNote('2026-10-10', '2026-10-10'), '');
  assert.equal(lockedDetailNote('2026-10-11', '2026-10-10'), '');
  for (const locked of [null, undefined, '']) assert.equal(lockedDetailNote('2026-10-02', locked), '');
  assert.equal(lockedDetailNote(null, '2026-10-10'), '');
});
