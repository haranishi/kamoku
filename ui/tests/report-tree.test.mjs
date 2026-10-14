// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { treeRows, flattenBsRows, DIRECT_LABEL, chartCompare } from '../js/report-tree.js';

const accounts = [
  { code: 'cash', name: '現金', kind: 'asset', sort: 100, parent: null },
  { code: 'dep_other', name: '普通預金・その他の預金', kind: 'asset', sort: 130, parent: null },
  { code: 'owner_draw', name: '事業主貸', kind: 'asset', sort: 260, parent: null },
  { code: 'w01', name: '現金', kind: 'asset', sort: 1001, parent: 'cash' },
  { code: 'w02', name: 'デモ銀行', kind: 'asset', sort: 1002, parent: 'dep_other' },
  { code: 'w04', name: 'デモ電子マネー', kind: 'asset', sort: 1004, parent: 'cash' },
  { code: 'accrued', name: '未払金', kind: 'liability', sort: 230, parent: null },
];

test('SC-06 親の科目の下に子を並べ、親の行に子の合計を出す（親が行に無ければ合計だけの行を作る）', () => {
  const rows = [
    { code: 'owner_draw', name: '事業主貸', closing: 400 },
    { code: 'w01', name: '現金', closing: 100 },
    { code: 'w02', name: 'デモ銀行', closing: 2000 },
    { code: 'w04', name: 'デモ電子マネー', closing: 30 },
  ];
  const out = treeRows(rows, accounts, ['closing']);
  assert.deepEqual(
    out.map((r) => [r.code, r.depth, r.parent, r.closing]),
    [
      ['cash', 0, true, 130],
      ['w01', 1, false, 100],
      ['w04', 1, false, 30],
      ['dep_other', 0, true, 2000],
      ['w02', 1, false, 2000],
      ['owner_draw', 0, false, 400],
    ],
  );
  // 親の名前は科目の名前を使う。
  assert.equal(out[0].name, '現金');
});

test('SC-06 親に直接の記帳があれば、それを子の行にして、親の行は子の行の合計と一致させる', () => {
  const rows = [
    { code: 'cash', name: '現金', opening: 5, debit: 10, credit: 0, closing: 15 },
    { code: 'w01', name: '現金', opening: 100, debit: 50, credit: 20, closing: 130 },
  ];
  const out = treeRows(rows, accounts, ['opening', 'debit', 'credit', 'closing']);
  assert.deepEqual(out.map((r) => [r.name, r.depth, r.direct, r.closing]), [
    ['現金', 0, false, 145],
    [DIRECT_LABEL, 1, true, 15],
    ['現金', 1, false, 130],
  ]);
  assert.equal(out[0].debit, 60);
  // 親の行は、子の行の合計と一致する（二重に数えない）。
  const kids = out.filter((r) => r.depth === 1);
  assert.equal(kids.reduce((a, r) => a + r.closing, 0), out[0].closing);
});

test('月別の列（12個の配列）も足す', () => {
  const m = (v) => Array.from({ length: 12 }, (_, i) => (i === 0 ? v : 0));
  const out = treeRows([{ code: 'w01', name: '現金', amount: 3, months: m(3) }, { code: 'w04', name: '電子', amount: 4, months: m(4) }], accounts, ['amount', 'months']);
  assert.equal(out[0].amount, 7);
  assert.equal(out[0].months[0], 7);
  assert.equal(out[0].months.length, 12);
});

test('科目の並びは、分類を先に比べる（sort は分類をまたいで重なる）', () => {
  const sorted = [...accounts].sort(chartCompare).map((a) => a.code);
  assert.deepEqual(sorted.slice(0, 3), ['cash', 'dep_other', 'owner_draw']);
  assert.equal(sorted.at(-1), 'accrued');
});

test('SC-06 貸借対照表の木: 親の金額はサーバーの合計、直接の記帳は子の行', () => {
  const rows = [{ code: 'cash', name: '現金', amount: 145, own: 15, children: [{ code: 'w01', name: '現金', amount: 130, own: 130 }] }, { code: 'owner_draw', name: '事業主貸', amount: 400, own: 400 }];
  assert.deepEqual(flattenBsRows(rows).map((r) => [r.name, r.depth, r.amount, r.parent]), [
    ['現金', 0, 145, true],
    [DIRECT_LABEL, 1, 15, false],
    ['現金', 1, 130, false],
    ['事業主貸', 0, 400, false],
  ]);
});
