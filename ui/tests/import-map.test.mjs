// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initialMapping, columnOptions, mappingProblems, describeSource, encodingLabel, delimiterLabel, countsText, fallbackMapping } from '../js/import-map.js';

const upload = {
  encoding: 'shift_jis',
  delimiter: ',',
  header: ['日付', '摘要', '出金', '入金', '残高'],
  sample: [['2026/10/01', 'サンプルストア', '1,650', '', '98,350']],
  guess: { skip_rows: 1, date_col: 0, desc_cols: [1], id_col: null, amount: { mode: 'split', out_col: 2, in_col: 3 } },
  saved: null,
};

test('AC-04.3 列の対応の初期値: 覚えさせた対応 → 推測 → 決まった形 の順', () => {
  assert.equal(initialMapping(upload).source, 'guess');
  const saved = { ...upload.guess, desc_cols: [1, 4] };
  const withSaved = initialMapping({ ...upload, saved });
  assert.equal(withSaved.source, 'saved');
  assert.deepEqual(withSaved.mapping.desc_cols, [1, 4]);
  // 列の数に合わない保存（別の形式のファイル）は使わない。
  assert.equal(initialMapping({ ...upload, saved: { ...saved, date_col: 9 } }).source, 'guess');
  const none = initialMapping({ ...upload, guess: null, header: null });
  assert.equal(none.source, 'none');
  assert.equal(none.mapping.amount.mode, 'signed');
  assert.match(describeSource('none'), /推測できませんでした/);
});

test('AC-04.3 初期値は元の推測を書き換えない（画面で直しても、推測の値は残る）', () => {
  const m = initialMapping(upload).mapping;
  m.desc_cols.push(4);
  m.amount.out_col = 4;
  assert.deepEqual(upload.guess.desc_cols, [1]);
  assert.equal(upload.guess.amount.out_col, 2);
});

test('AC-04.3 列の選択肢の名前は、見出しと見本の値', () => {
  const o = columnOptions(upload);
  assert.equal(o.length, 5);
  assert.equal(o[2].label, '3列目「出金」（例: 1,650）');
  assert.equal(o[3].label, '4列目「入金」');
  const bare = columnOptions({ header: null, sample: [['20261001', 'カフェ', '880']] });
  assert.equal(bare[0].label, '1列目（例: 20261001）');
});

test('AC-04.3 誤った対応を見つける（出金と入金が同じ列、日付と金額が同じ列、範囲の外）', () => {
  assert.deepEqual(mappingProblems(upload.guess, 5), []);
  const same = { ...upload.guess, amount: { mode: 'split', out_col: 2, in_col: 2 } };
  assert.ok(mappingProblems(same, 5).some((p) => p.field === 'amount'));
  const clash = { ...upload.guess, amount: { mode: 'signed', col: 0, inflow: 'positive' } };
  assert.ok(mappingProblems(clash, 5).some((p) => p.field === 'date_col'));
  assert.ok(mappingProblems({ ...upload.guess, date_col: 7 }, 5).some((p) => p.field === 'date_col'));
  assert.ok(mappingProblems({ ...upload.guess, skip_rows: -1 }, 5).some((p) => p.field === 'skip_rows'));
});

test('表示名と件数の文', () => {
  assert.equal(encodingLabel('shift_jis'), 'Shift_JIS');
  assert.equal(encodingLabel('utf-8'), 'UTF-8');
  assert.equal(delimiterLabel('\t'), 'タブ');
  assert.equal(countsText({ new: 300, duplicate: 10, invalid: 2 }), '新規 300件・重複 10件・読めない 2件');
  assert.equal(fallbackMapping({ header: null, sample: [['a']] }).date_col, 0);
});
