// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { accountItems, walletParentCodes } from '../js/screens/entry-options.js';

const acc = (code, name, kind, extra = {}) => ({ code, name, reading: name, kind, enabled: true, sort: 0, wallet: null, parent: null, ...extra });

// 標準の科目と、口座を足した帳簿（現金・預金・カードの口座）。
const ACCOUNTS = [
  acc('cash', '現金', 'asset'),
  acc('dep_other', '普通預金・その他の預金', 'asset'),
  acc('accrued', '未払金', 'liability'),
  acc('exp_comm', '通信費', 'expense'),
  acc('w01', '現金', 'asset', { parent: 'cash', wallet: { type: 'cash', institution: '' } }),
  acc('w02', 'サンプル銀行 普通預金', 'asset', { parent: 'dep_other', wallet: { type: 'bank', institution: 'サンプル銀行' } }),
  acc('w03', 'サンプルカード', 'liability', { parent: 'accrued', wallet: { type: 'card', institution: 'サンプルカード' } }),
];
const codes = (items) => items.map((i) => i.value);

test('AC-02.9 口座の親の科目は、現金と預金だけ（カードの親の未払金は入れない）', () => {
  assert.deepEqual([...walletParentCodes(ACCOUNTS)].sort(), ['cash', 'dep_other']);
});

test('AC-02.9 記帳の科目の候補から、口座の親（現金・預金）を外す。口座と費用の科目は残る', () => {
  const entry = codes(accountItems(ACCOUNTS, 'expense', { forEntry: true }));
  assert.ok(!entry.includes('cash'), '親の「現金」は出さない');
  assert.ok(!entry.includes('dep_other'), '親の「預金」は出さない');
  for (const code of ['w01', 'w02', 'w03', 'accrued', 'exp_comm']) assert.ok(entry.includes(code), `${code} は残る`);
  // 同じ名前の「現金」が2つ並ばない。
  assert.equal(accountItems(ACCOUNTS, 'expense', { forEntry: true }).filter((i) => i.label === '現金').length, 1);
});

test('AC-02.9 絞り込みの欄など、記帳でない候補では親も選べる', () => {
  const filter = codes(accountItems(ACCOUNTS, 'expense'));
  assert.ok(filter.includes('cash') && filter.includes('dep_other'));
});

test('AC-02.9 訂正で、元の仕訳が親の科目を使っているときは、その科目を候補に残す', () => {
  const kept = codes(accountItems(ACCOUNTS, 'expense', { forEntry: true, keep: ['cash', null] }));
  assert.ok(kept.includes('cash'));
  assert.ok(!kept.includes('dep_other'), 'keep に無い親は外れたまま');
});

test('AC-02.9 口座が無い帳簿では、現金も預金も親の科目をそのまま選べる', () => {
  const noWallets = ACCOUNTS.filter((a) => !a.wallet);
  assert.equal(walletParentCodes(noWallets).size, 0);
  const items = codes(accountItems(noWallets, 'expense', { forEntry: true }));
  assert.ok(items.includes('cash') && items.includes('dep_other'));
});

test('AC-02.9 口座を無効にしたら、その口座の親はまた選べる', () => {
  const off = ACCOUNTS.map((a) => (a.code === 'w01' ? { ...a, enabled: false } : a));
  const items = codes(accountItems(off, 'expense', { forEntry: true }));
  assert.ok(items.includes('cash'), '現金の口座が無効なら、親の現金が候補に戻る');
  assert.ok(!items.includes('dep_other'));
});
