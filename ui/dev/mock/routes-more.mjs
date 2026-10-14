// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// U2 の画面に要る応え（未確認・取込・帳票・書き出し・科目と取引先の追加と変更・期首残高）。
// 形は docs/spec/api-*.md と同じにする。routes.mjs の ROUTES の後ろに足される。

import { upload, preview, commit } from './imports.mjs';
import { listDrafts, patchDraft, ignoreDraft, restoreDraft, confirmDrafts } from './drafts.mjs';
import { trialBalance, incomeStatement, balanceSheet, accountLedger, taxSummary } from './books.mjs';
import { addAccount, patchAccount, addPartner, patchPartner, getOpening, putOpening } from './masters.mjs';
import { exportFile } from './exports.mjs';

export const MORE_ROUTES = [
  ['POST', /^\/accounts$/, ({ store, body }) => [201, addAccount(store, body)]],
  ['PATCH', /^\/accounts\/([a-z0-9_]+)$/, ({ store, m, body }) => [200, patchAccount(store, m[1], body)]],
  ['POST', /^\/partners$/, ({ store, body }) => [201, addPartner(store, body)]],
  ['PATCH', /^\/partners\/(\d+)$/, ({ store, m, body }) => [200, patchPartner(store, m[1], body)]],
  ['GET', /^\/opening$/, ({ store }) => [200, getOpening(store)]],
  ['PUT', /^\/opening$/, ({ store, body }) => [200, putOpening(store, body)]],

  ['POST', /^\/imports\/upload$/, ({ store, q, raw }) => [200, upload(store, q, raw)]],
  ['POST', /^\/imports\/preview$/, ({ store, body }) => [200, preview(store, body)]],
  ['POST', /^\/imports\/commit$/, ({ store, body }) => [200, commit(store, body)]],
  ['GET', /^\/imports\/profile$/, ({ store, q }) => [200, store.db.profiles.get(q.wallet) || null]],
  ['GET', /^\/imports\/batches$/, ({ store }) => [200, { items: [...store.db.batches].reverse(), total: store.db.batches.length }]],

  ['GET', /^\/drafts$/, ({ store, q }) => [200, listDrafts(store, q)]],
  ['POST', /^\/drafts\/confirm$/, ({ store, body }) => [200, confirmDrafts(store, body)]],
  ['PATCH', /^\/drafts\/(\d+)$/, ({ store, m, body }) => [200, patchDraft(store, m[1], body)]],
  ['POST', /^\/drafts\/(\d+)\/ignore$/, ({ store, m }) => [200, ignoreDraft(store, m[1])]],
  ['POST', /^\/drafts\/(\d+)\/restore$/, ({ store, m }) => [200, restoreDraft(store, m[1])]],

  ['GET', /^\/reports\/trial-balance$/, ({ store, q }) => [200, trialBalance(store, q)]],
  ['GET', /^\/reports\/income-statement$/, ({ store, q }) => [200, incomeStatement(store, q)]],
  ['GET', /^\/reports\/balance-sheet$/, ({ store, q, today }) => [200, balanceSheet(store, q, today)]],
  ['GET', /^\/reports\/ledger$/, ({ store, q }) => [200, accountLedger(store, q)]],
  ['GET', /^\/reports\/tax-summary$/, ({ store, q }) => [200, taxSummary(store, q)]],

  ['GET', /^\/export\/([a-z-]+\.(?:csv|json|db))$/, ({ store, m, q, today }) => [200, exportFile(store, m[1], q, today)]],
];

/** 書き込みでも、読み取り専用のときに通すもの（台帳に何も書かない）。 */
export const MORE_NOT_WRITES = ['/imports/upload', '/imports/preview'];
