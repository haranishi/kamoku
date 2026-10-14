// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面どうしで共有する読み込み（状態・設定・科目・取引先・テンプレート）。
// 同じものを何度も取りに行かないよう、読み込み中の約束（Promise）をまとめる。書き込んだら invalidate する。

import { api, onApiError } from './api.js';
import { ledgerErrorKind } from './ledger-check.js';

const LOADERS = {
  status: () => api.get('/status'),
  settings: () => api.get('/settings'),
  // 無効な科目も読む（過去の仕訳の名前を出すため）。候補には有効なものだけを使う。
  accounts: () => api.get('/accounts', { include_disabled: 1 }).then((r) => r.items),
  partners: () => api.get('/partners', { limit: 500 }).then((r) => r.items),
  templates: () => api.get('/templates').then((r) => r.items),
};

const values = new Map();
const inflight = new Map();
const subs = new Map();
let readOnlyFromServer = false;

function notify(key) {
  (subs.get(key) || new Set()).forEach((fn) => fn(values.get(key)));
}

/** 読み込み済みの値（無ければ undefined）。 */
export function peek(key) {
  return values.get(key);
}

/**
 * 値を読み込む。読み込み済みならそれを返す。force で取り直す。
 * @param {'status'|'settings'|'accounts'|'partners'|'templates'} key
 */
export function load(key, { force = false } = {}) {
  if (!force && values.has(key)) return Promise.resolve(values.get(key));
  if (inflight.has(key)) return inflight.get(key);
  const p = LOADERS[key]()
    .then((v) => {
      values.set(key, v);
      notify(key);
      return v;
    })
    .finally(() => inflight.delete(key));
  inflight.set(key, p);
  return p;
}

/** 値を直接入れる（保存の応えを使い回すとき）。 */
export function put(key, value) {
  values.set(key, value);
  notify(key);
}

/** 次の load で取り直すようにする。 */
export function invalidate(...keys) {
  keys.forEach((k) => values.delete(k));
}

/** 値が変わったら呼ぶ関数を登録する。片付けの関数を返す。 */
export function subscribe(key, fn) {
  if (!subs.has(key)) subs.set(key, new Set());
  subs.get(key).add(fn);
  return () => subs.get(key).delete(fn);
}

/** 読み取り専用か（起動時の検証の失敗、または書き込みが read_only で断られた）。 */
export function isReadOnly() {
  const st = values.get('status');
  return readOnlyFromServer || Boolean(st && st.ledger && st.ledger.verified === false);
}

/** 読み取り専用の理由（画面に出す文）。 */
export function readOnlyReason() {
  const st = values.get('status');
  const err = st && st.ledger && st.ledger.error;
  if (err) {
    // 種類の名前は、知らない値でも英字を出さない（ledger-check.js の表。U2が本物のサーバーの値に合わせて足した）。
    const what = ledgerErrorKind(err.kind);
    return `台帳の検証で、${err.seq}番目の記録に${what}が見つかりました。読み取り専用で開いているため、書き込みはできません。`;
  }
  return '台帳が読み取り専用のため、書き込みはできません。';
}

// 書き込みが read_only で断られたら、その場で読み取り専用に切り替え、状態を取り直す。
onApiError((err) => {
  if (err.isReadOnly && !readOnlyFromServer) {
    readOnlyFromServer = true;
    load('status', { force: true }).catch(() => notify('status'));
  }
});

/** 科目のコードから科目を引く表。 */
export function accountMap() {
  return new Map((values.get('accounts') || []).map((a) => [a.code, a]));
}
