// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { treeRows, flattenBsRows, DIRECT_LABEL, chartCompare, collapseSingleChildren, collapseSingleRows, monthColumns, monthList, hiddenMonthsNote } from '../js/report-tree.js';

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

// 貸借対照表と試算表で、子が1つだけの親を1行に畳む（「現金／現金」と同じ金額が2回並ばないように）。
test('SC-06 子が1つだけの親は1行に畳み、子の名前が親と違えば sub に残す', () => {
  const rows = [
    { code: 'cash', name: '現金', amount: 49770, own: 0, children: [{ code: 'w01', name: '現金', amount: 49770, own: 49770 }] },
    { code: 'dep_other', name: '普通預金・その他の預金', amount: 4622031, own: 0, children: [{ code: 'w02', name: 'サンプル銀行 普通預金', amount: 4622031, own: 4622031 }] },
    { code: 'owner_draw', name: '事業主貸', amount: 449796, own: 449796 },
  ];
  const out = collapseSingleChildren(rows);
  assert.deepEqual(out.map((r) => [r.code, r.name, r.amount, r.sub, (r.children || []).length]), [
    ['cash', '現金', 49770, undefined, 0],
    ['dep_other', '普通預金・その他の預金', 4622031, 'サンプル銀行 普通預金', 0],
    ['owner_draw', '事業主貸', 449796, undefined, 0],
  ]);
  // 平らにしても1行ずつ（親の行と子の行が同じ金額で2回出ない）。
  assert.deepEqual(flattenBsRows(out).map((r) => [r.name, r.sub, r.depth, r.parent]), [
    ['現金', undefined, 0, false],
    ['普通預金・その他の預金', 'サンプル銀行 普通預金', 0, false],
    ['事業主貸', undefined, 0, false],
  ]);
  // 元の木は変えない。
  assert.equal(rows[0].children.length, 1);
  assert.equal(rows[1].sub, undefined);
});

test('SC-06 親に直接の記帳があるとき、子が2つ以上のとき、子が末端でないときは畳まない', () => {
  const own = { code: 'cash', name: '現金', amount: 145, own: 15, children: [{ code: 'w01', name: '現金', amount: 130, own: 130 }] };
  const two = { code: 'cash', name: '現金', amount: 130, own: 0, children: [{ code: 'w01', name: '現金', amount: 100, own: 100 }, { code: 'w04', name: '電子マネー', amount: 30, own: 30 }] };
  const deep = {
    code: 'a',
    name: '親',
    amount: 30,
    own: 0,
    children: [{ code: 'b', name: '子', amount: 30, own: 0, children: [{ code: 'c', name: '孫1', amount: 10, own: 10 }, { code: 'd', name: '孫2', amount: 20, own: 20 }] }],
  };
  const [o, t, d] = collapseSingleChildren([own, two, deep]);
  assert.equal(o.children.length, 1);
  assert.equal(o.sub, undefined);
  assert.equal(t.children.length, 2);
  assert.equal(d.children.length, 1);
  assert.equal(d.children[0].children.length, 2);
});

test('SC-06 孫まで1本道なら、途中の名前もつないで sub に残す（親と同じ名前は省く）', () => {
  const rows = [{ code: 'a', name: '未払金', amount: 9, own: 0, children: [{ code: 'b', name: '未払金', amount: 9, own: 0, children: [{ code: 'c', name: 'サンプルカード', amount: 9, own: 9 }] }] }];
  const [r] = collapseSingleChildren(rows);
  assert.equal(r.children.length, 0);
  assert.equal(r.sub, 'サンプルカード');
  assert.equal(r.amount, 9);
});

// 月次推移の「科目ごとの月別」の列（設計担当の追加の指摘 1・2）。
const months12 = (year) => Array.from({ length: 12 }, (_, i) => ({ month: `${year}-${String(i + 1).padStart(2, '0')}` }));
const at = (i, v) => Array.from({ length: 12 }, (_, k) => (k === i ? v : 0));

test('SC-06 振替だけの月（収益と費用の合計が0）でも、科目の金額があれば「金額がある」月にする', () => {
  const rows = [
    { code: 'sales', months: at(0, 1000) },
    // 7月に、費用A +10,000 と 費用B −10,000（費用の合計は0）。
    { code: 'exp_comm', months: at(6, 10000) },
    { code: 'exp_supplies', months: at(6, -10000) },
  ];
  const cols = monthColumns(months12(2026), rows, '2026-10');
  assert.deepEqual(cols.map((c) => c.month), ['2026-01', '2026-02', '2026-03', '2026-04', '2026-05', '2026-06', '2026-07', '2026-08', '2026-09', '2026-10']);
  assert.equal(cols.find((c) => c.month === '2026-07').hasAmounts, true);
  assert.equal(cols.find((c) => c.month === '2026-01').hasAmounts, true);
  assert.equal(cols.find((c) => c.month === '2026-02').hasAmounts, false);
});

test('SC-06 今月より後の月も、科目の金額がある月は列に出す（列を足すと、年間の合計と合う）', () => {
  const rows = [
    { code: 'sales', amount: 1000, months: at(2, 1000) },
    { code: 'exp_comm', amount: 1234 + 500, months: at(10, 1234).map((v, i) => (i === 4 ? 500 : v)) },
  ];
  const cols = monthColumns(months12(2026), rows, '2026-10');
  // 1〜10月と、金額のある11月。金額の無い12月は出さない。
  assert.deepEqual(cols.map((c) => c.month).slice(-2), ['2026-10', '2026-11']);
  assert.equal(cols.length, 11);
  for (const r of rows) assert.equal(cols.reduce((a, c) => a + r.months[c.i], 0), r.amount);
  // 来年を見ている（until が前の年の末より前）ときは、金額のある月だけ。
  assert.deepEqual(monthColumns(months12(2027), rows, '2027-00').map((c) => c.month), ['2027-03', '2027-05', '2027-11']);
});

// 試算表・損益計算書の平らな行でも、貸借対照表と同じく、子が1つだけの親を1行に畳む。
test('SC-06 平らな行: 子が1つだけの親は1行に畳む（数字は親の行のまま、子の名前は sub）', () => {
  const keys = ['opening', 'debit', 'credit', 'closing'];
  const flat = treeRows(
    [
      { code: 'w01', name: '現金', opening: 30, debit: 90, credit: 70, closing: 50 },
      { code: 'w02', name: 'デモ銀行', opening: 1200, debit: 5500, credit: 2080, closing: 4620 },
      { code: 'owner_draw', name: '事業主貸', opening: 0, debit: 450, credit: 0, closing: 450 },
    ],
    accounts,
    keys,
  );
  assert.equal(flat.length, 5);
  const out = collapseSingleRows(flat);
  assert.deepEqual(out.map((r) => [r.code, r.name, r.sub, r.depth, r.parent, r.closing, r.debit]), [
    ['cash', '現金', undefined, 0, false, 50, 90],
    ['dep_other', '普通預金・その他の預金', 'デモ銀行', 0, false, 4620, 5500],
    ['owner_draw', '事業主貸', undefined, 0, false, 450, 450],
  ]);
  // 元の行は変えない。
  assert.equal(flat[0].parent, true);
});

test('SC-06 平らな行: 子が2つ以上、または直接の記帳の行がある親は畳まない。孫まで1本道なら名前をつなぐ', () => {
  const two = treeRows([{ code: 'w01', name: '現金', closing: 100 }, { code: 'w04', name: 'デモ電子マネー', closing: 30 }], accounts, ['closing']);
  assert.deepEqual(collapseSingleRows(two).map((r) => r.code), ['cash', 'w01', 'w04']);
  const direct = treeRows([{ code: 'cash', name: '現金', closing: 15 }, { code: 'w01', name: '現金', closing: 130 }], accounts, ['closing']);
  assert.deepEqual(collapseSingleRows(direct).map((r) => [r.name, r.direct]), [['現金', false], [DIRECT_LABEL, true], ['現金', false]]);
  const chain = [
    { code: 'a', name: '未払金', depth: 0, parent: true, direct: false, amount: 9 },
    { code: 'b', name: 'カード', depth: 1, parent: true, direct: false, amount: 9 },
    { code: 'c', name: 'サンプルカード', depth: 2, parent: false, direct: false, amount: 9 },
    { code: 'd', name: '事業主借', depth: 0, parent: false, direct: false, amount: 1 },
  ];
  assert.deepEqual(collapseSingleRows(chain).map((r) => [r.code, r.sub, r.depth, r.parent, r.amount]), [
    ['a', 'カード・サンプルカード', 0, false, 9],
    ['d', undefined, 0, false, 1],
  ]);
});

// SC-06 月次推移: グラフは12月まであるのに、表が今月で終わる理由が画面に無かった（見た目の採点 第2回）。表の直前に、
// 表に出ない月と、出る条件を書く。今月より後の月を表に出しているときは「表は今月までです」を付けない（表の題と食い違うため）。
test('SC-06 月の並びの言葉: 2か月だけなら「と」、3か月以上続く所は「〜」、ほかは「・」', () => {
  assert.equal(monthList([12]), '12月');
  assert.equal(monthList([11, 12]), '11月と12月');
  assert.equal(monthList([4, 5, 6, 7]), '4月〜7月');
  assert.equal(monthList([4, 6]), '4月と6月');
  assert.equal(monthList([2, 3, 5]), '2月・3月・5月');
  assert.equal(monthList([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12]), '1月〜10月・12月');
});

test('SC-06 月次推移の表に出ない月の一言: 今年・今月まで・後の月を出しているとき・来年・去年', () => {
  const none = [{ code: 'sales', months: at(1, 1000) }];
  // 今年の10月: 11月と12月が表に出ない。
  assert.equal(hiddenMonthsNote(months12(2026), monthColumns(months12(2026), none, '2026-10'), '2026-10'), '表は今月までです。11月と12月は、その月になるか、記録ができたら出ます。');
  // 12月には、出ない月が無い。
  assert.equal(hiddenMonthsNote(months12(2026), monthColumns(months12(2026), none, '2026-12'), '2026-12'), '');
  // 11月に記録があると、11月は表に出る（今月より後の月を出しているので、「表は今月までです」は付けない）。
  const nov = [{ code: 'exp_comm', months: at(10, 1234) }];
  assert.equal(hiddenMonthsNote(months12(2026), monthColumns(months12(2026), nov, '2026-10'), '2026-10'), '12月は、その月になるか、記録ができたら出ます。');
  // 来年（金額のある月だけ）。
  assert.equal(hiddenMonthsNote(months12(2027), monthColumns(months12(2027), nov, '2027-00'), '2026-10'), '1月〜10月・12月は、その月になるか、記録ができたら出ます。');
  // 記録の無い来年（「表は今月までです」は付けない）。
  assert.equal(hiddenMonthsNote(months12(2027), monthColumns(months12(2027), [], '2027-00'), '2026-10'), '1月〜12月は、その月になるか、記録ができたら出ます。');
  // 去年（12か月そろう）。
  assert.equal(hiddenMonthsNote(months12(2025), monthColumns(months12(2025), none, '2025-12'), '2026-10'), '');
});
