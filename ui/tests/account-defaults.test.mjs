// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { nextDefaults, taxOptions, isTransitional } from '../js/account-defaults.js';

const COMM = { code: 'exp_comm', default_tax: 'P10', default_pct: 70, wallet: null };
const VEHICLE = { code: 'exp_vehicle', default_tax: 'P10', default_pct: 30, wallet: null };
const RENT = { code: 'exp_rent', default_tax: null, default_pct: 100, wallet: null };
const CARD = { code: 'w03', default_tax: null, default_pct: 100, wallet: { type: 'card' } };

test('AC-03.3 科目を選ぶと、その科目の既定の税区分と割合になる', () => {
  assert.deepEqual(nextDefaults({ prev: null, next: COMM, tax: null, pct: 100 }), { tax: 'P10', pct: 70, locked: false });
});

test('AC-03.3 いまの値が前の科目の既定値と同じなら、新しい科目の既定値に変える', () => {
  assert.deepEqual(nextDefaults({ prev: COMM, next: VEHICLE, tax: 'P10', pct: 70 }), { tax: 'P10', pct: 30, locked: false });
  assert.deepEqual(nextDefaults({ prev: COMM, next: RENT, tax: 'P10', pct: 70 }), { tax: null, pct: 100, locked: false });
});

test('AC-03.3 利用者が変えた値（前の科目の既定値と違う）は残す', () => {
  assert.deepEqual(nextDefaults({ prev: COMM, next: VEHICLE, tax: 'P10', pct: 50 }), { tax: 'P10', pct: 50, locked: false });
  assert.deepEqual(nextDefaults({ prev: COMM, next: RENT, tax: 'P8', pct: 70 }), { tax: 'P8', pct: 100, locked: false });
});

test('AC-02.8 相手科目が口座なら、税区分は対象外・割合は100に固める', () => {
  assert.deepEqual(nextDefaults({ prev: COMM, next: CARD, tax: 'P10', pct: 50 }), { tax: null, pct: 100, locked: true });
  // 口座から普通の科目に戻したときは、口座を「既定値が空・100」の科目とみなして、新しい既定値を入れる。
  assert.deepEqual(nextDefaults({ prev: CARD, next: COMM, tax: null, pct: 100 }), { tax: 'P10', pct: 70, locked: false });
});

test('税区分の選択肢は、支出なら仕入、収入なら売上', () => {
  assert.deepEqual(taxOptions('expense').map((o) => o.value), ['', 'P10', 'P8', 'P10X', 'P8X', 'NT']);
  assert.deepEqual(taxOptions('income').map((o) => o.value), ['', 'S10', 'S8', 'NT']);
  assert.equal(isTransitional('P10X'), true);
  assert.equal(isTransitional('P10'), false);
});
