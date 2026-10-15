// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 訂正の画面と仕訳の詳細の板の、戻り先の決まり（journal-return.js）。2026-10-15 の体験の評価（E1 の B5、E3 の7、E4 の1）。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { isReturnTarget, sameAddress, toBackParam, fromBackParam, correctHash, detailHash } from '../js/screens/journal-return.js';

test('SC-05 板を閉じて戻る画面: 元帳やホームへは戻る。仕訳帳・登録と訂正・初期設定・アプリの外は戻り先にしない', () => {
  assert.equal(isReturnTarget('#/reports/ledger?account=exp_supplies&month=2026-10'), true);
  assert.equal(isReturnTarget('#/'), true);
  for (const h of ['#/journal', '#/journal?q=a', '#/journal/e_1', '#/entry', '#/entry/e_1?back=%2Fjournal', '#/setup', '', null, 'https://example.com/#/', '/reports']) {
    assert.equal(isReturnTarget(h), false, String(h));
  }
});

test('SC-05 同じアドレスか: 条件の並びと文字の符号の違いは見ない。画面や条件の値が違えば別', () => {
  assert.equal(sameAddress('#/journal?q=%E6%90%BA%E5%B8%AF&from=2026-10-01', '#/journal?from=2026-10-01&q=携帯'), true);
  assert.equal(sameAddress('#/journal', '#/journal'), true);
  assert.equal(sameAddress('#/journal?q=a', '#/journal?q=b'), false);
  assert.equal(sameAddress('#/journal?q=a', '#/journal/e_1?q=a'), false);
});

test('SC-05 back の書き方: 先頭の # を外し、読むときに戻す。条件の無い仕訳帳は書かない。アプリの外の道は読まない', () => {
  assert.equal(toBackParam('#/journal?q=a'), '/journal?q=a');
  assert.equal(toBackParam('#/journal'), null);
  assert.equal(toBackParam(null), null);
  assert.equal(fromBackParam('/reports/ledger?account=x'), '#/reports/ledger?account=x');
  for (const v of ['//example.com', 'https://example.com', 'javascript:alert(1)', '', undefined]) assert.equal(fromBackParam(v), null, String(v));
});

test('SC-05 back は、戻り先にしてよい道だけ読む。訂正・詳細・初期設定・読めない符号は受けない（アドレスを書き換えられても、おかしな所へ飛ばない）', () => {
  assert.equal(fromBackParam('/'), '#/');
  assert.equal(fromBackParam('/journal?q=a'), '#/journal?q=a');
  assert.equal(fromBackParam('/reports/ledger?account=x'), '#/reports/ledger?account=x');
  for (const v of ['/entry', '/entry/e_1', '/journal/e_1', '/setup', '/reports/%', '/reports/%E0%A4%A']) assert.equal(fromBackParam(v), null, v);
});

test('AC-06.4 訂正の画面のアドレスに、詳細を閉じたときの戻り先を持たせる（条件の無い一覧なら持たせない）', () => {
  assert.equal(correctHash('e_1', '#/journal'), '#/entry/e_1');
  assert.equal(correctHash('e_1', null), '#/entry/e_1');
  const h = correctHash('e_1', '#/journal?q=携帯');
  assert.match(h, /^#\/entry\/e_1\?back=/);
  assert.ok(sameAddress(fromBackParam(new URLSearchParams(h.split('?')[1]).get('back')), '#/journal?q=%E6%90%BA%E5%B8%AF'));
});

test('AC-06.4 訂正の後の詳細: 一覧が戻り先なら、その条件のまま一覧の上に開く。ほかの画面なら back に持ち回る', () => {
  assert.equal(detailHash('e_1', null), '#/journal/e_1');
  assert.equal(detailHash('e_1', '#/journal?q=%E6%90%BA%E5%B8%AF&page=2'), '#/journal/e_1?q=%E6%90%BA%E5%B8%AF&page=2');
  const ledger = '#/reports/ledger?account=exp_supplies&month=2026-10';
  const h = detailHash('e_1', ledger);
  assert.match(h, /^#\/journal\/e_1\?back=/);
  assert.equal(fromBackParam(new URLSearchParams(h.split('?')[1]).get('back')), ledger);
});
