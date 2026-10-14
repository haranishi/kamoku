// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createCandidates } from '../js/candidates.js';
import { normalizeHash, notedText, verifyText, ledgerErrorKind } from '../js/ledger-check.js';
import { normalizeInvoice } from '../js/invoice.js';

test('AC-05.3 確定候補の件数と、入金の合計・出金の合計を分けて出す', () => {
  const c = createCandidates();
  c.add({ id: 1, direction: 'out', amount: 1650 });
  c.add({ id: 2, direction: 'out', amount: 420 });
  c.add({ id: 3, direction: 'in', amount: 55000 });
  assert.deepEqual(c.summary(), { count: 3, inTotal: 55000, outTotal: 2070 });
  // 同じ行に2回付けても1件。外すと合計から抜ける。
  c.add({ id: 1, direction: 'out', amount: 1650 });
  assert.equal(c.size, 3);
  assert.equal(c.toggle({ id: 2, direction: 'out', amount: 420 }), false);
  assert.deepEqual(c.summary(), { count: 2, inTotal: 55000, outTotal: 1650 });
  assert.equal(c.toggle({ id: 2, direction: 'out', amount: 420 }), true);
  assert.deepEqual(c.ids().sort(), [1, 2, 3]);
});

test('AC-09.2 控えたハッシュは、空白・改行・大文字を許して64桁の16進にそろえる', () => {
  const h = 'AB'.repeat(32);
  assert.deepEqual(normalizeHash(` ${h.slice(0, 30)}\n${h.slice(30)} `), { ok: true, value: h.toLowerCase() });
  assert.match(normalizeHash('').error, /入れてください/);
  assert.match(normalizeHash('xyz').error, /0〜9 と a〜f/);
  assert.match(normalizeHash('ab12').error, /64文字です（いまは4文字）/);
});

test('AC-09.2 照合の結果の文: 何番目にあたるか、末尾か、見つからないか', () => {
  assert.match(notedText({ found: true, seq: 128, is_head: true }, 128).text, /128番目の記録と一致しました（末尾と同じです）/);
  const mid = notedText({ found: true, seq: 100, is_head: false }, 128).text;
  assert.match(mid, /100番目の記録と一致しました（末尾ではありません）/);
  assert.match(mid, /28件の記録が足されています/);
  assert.equal(notedText({ found: false, seq: null, is_head: false }, 128).found, false);
});

test('AC-09.1 検査の結果の文: 壊れていれば最初の連番と規則を示す', () => {
  assert.equal(verifyText({ ok: true, events: 80 }).ok, true);
  const bad = verifyText({ ok: false, error: { seq: 55, kind: 'bad_hash', rule: null } });
  assert.match(bad.body, /55番目の記録に、ハッシュの不一致/);
  assert.match(verifyText({ ok: false, error: { seq: 7, kind: 'bad_rule', rule: 'unbalanced' } }).body, /決まり: unbalanced/);
});

test('AC-09.1 壊れの種類は、本物のサーバーの値（保存の層）を日本語にし、知らない値でも英字を出さない', () => {
  const kinds = { bad_hash: 'ハッシュの不一致', broken_link: '前の記録とのつながりの切れ', wrong_sequence: '連番の飛びや入れ違い', rule_violation: '記録の決まりへの違反', quick_check: 'データベースのファイルの傷み' };
  for (const [kind, text] of Object.entries(kinds)) assert.equal(ledgerErrorKind(kind), text);
  assert.equal(ledgerErrorKind('something_new'), '記録の不整合');
  assert.equal(ledgerErrorKind(undefined), '記録の不整合');
  const body = verifyText({ ok: false, error: { seq: 12, kind: 'broken_link', rule: null } }).body;
  assert.match(body, /12番目の記録に、前の記録とのつながりの切れ/);
  assert.doesNotMatch(verifyText({ ok: false, error: { seq: 3, kind: 'something_new', rule: null } }).body, /[a-z_]{4,}/);
});

test('SC-09 登録番号は全角・小文字・ハイフンを許して T+13桁にそろえ、形が違えば理由を返す', () => {
  assert.deepEqual(normalizeInvoice('ｔ１２３４－５６７８９０１２３'), { ok: true, value: 'T1234567890123' });
  assert.deepEqual(normalizeInvoice(''), { ok: true, value: null });
  assert.match(normalizeInvoice('T12345').error, /T と13桁/);
  assert.match(normalizeInvoice('1234567890123').error, /T と13桁/);
  // 桁の数の境目（12桁と14桁）も断る。
  assert.match(normalizeInvoice('T123456789012').error, /T と13桁/);
  assert.match(normalizeInvoice('T12345678901234').error, /T と13桁/);
});
