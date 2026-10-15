// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳の詳細の板を閉じたときの戻り先と、戻った画面でのフォーカス（担当をまたぐ約束 C1・C2）。
// 板は、開く前の画面へ戻す（総勘定元帳から開いたら、同じ科目・期間の元帳へ）。元帳から開いた仕訳を閉じると、
// 仕訳帳の一覧へ移り、元帳の条件もフォーカスも失われた（2026-10-15 の体験の評価）。
// 訂正を挟むとき（詳細 → 訂正の画面 → 保存した後の詳細）は、戻り先をアドレスの back に持ち回る。

import { parseHash, buildHash } from '../router.js';

/** 仕訳帳の一覧（条件なし）。ここへ戻るときは、back を書かない（アドレスを短く保つ）。 */
const PLAIN_LIST = '#/journal';

/** 戻り先にする画面か。仕訳帳（一覧は別の決まりで戻る）・登録と訂正（保存の後の詳細なので戻らない）・初期設定は除く。 */
export function isReturnTarget(hash) {
  if (typeof hash !== 'string' || !hash.startsWith('#/')) return false;
  const { path } = parseHash(hash);
  return !/^\/(journal|entry)(\/|$)/.test(path) && path !== '/setup';
}

/** 同じ画面・同じ条件のアドレスか（文字の符号の書き方や、条件の並びの違いは見ない）。 */
export function sameAddress(a, b) {
  const norm = (hash) => {
    const { path, query } = parseHash(hash || '');
    return `${path}?${Object.keys(query)
      .sort()
      .map((k) => `${k}=${query[k]}`)
      .join('&')}`;
  };
  return norm(a) === norm(b);
}

/** アドレスの back に入れる形（先頭の # を外す）。条件のない仕訳帳なら null（書かない）。 */
export function toBackParam(hash) {
  if (!hash || sameAddress(hash, PLAIN_LIST)) return null;
  return hash.replace(/^#/, '');
}

/**
 * back の値を、戻り先のアドレスに戻す。戻り先にしてよいのは、元帳やホームなど（isReturnTarget）と、仕訳帳の一覧だけ。
 * アプリの外の道、訂正・詳細・初期設定、読めない符号（経路の照合で例外になる）は、アドレスを書き換えられても受けない（null）。
 */
export function fromBackParam(value) {
  if (typeof value !== 'string' || !/^\/(?!\/)/.test(value)) return null;
  const hash = `#${value}`;
  try {
    decodeURIComponent(parseHash(hash).path);
    return isReturnTarget(hash) || isJournalList(hash) ? hash : null;
  } catch {
    return null;
  }
}

/** 仕訳帳の一覧（条件つきも含む）のアドレスか。 */
export function isJournalList(hash) {
  return parseHash(hash || '').path === '/journal';
}

/** 訂正の画面のアドレス。back は、訂正の後に詳細を閉じたときの戻り先。 */
export function correctHash(id, back) {
  return buildHash(`/entry/${encodeURIComponent(id)}`, { back: toBackParam(back) });
}

/**
 * 訂正を保存した後（とやめた後）の、詳細のアドレス。戻り先が仕訳帳の一覧なら、その条件のまま一覧の上に開く
 * （閉じると、検索の条件が残った一覧へ戻る）。ほかの画面なら back に持ち回る。
 */
export function detailHash(id, back) {
  const path = `/journal/${encodeURIComponent(id)}`;
  if (!back) return buildHash(path);
  if (isJournalList(back)) return buildHash(path, parseHash(back).query);
  return buildHash(path, { back: toBackParam(back) });
}

/** 表の中で、Tab で止まる最初の要素（見えているもの）。 */
function firstFocusable(root) {
  return [...root.querySelectorAll('table *')].find((el) => el.tabIndex >= 0 && !el.disabled && el.getClientRects().length > 0) || null;
}

/**
 * 戻った画面で、その仕訳の行（tr[data-entry-id]。約束 C1）が出るまで待ち、フォーカスを移す（約束 C4）。
 * 画面が準備できるまで、最大 timeout ミリ秒待つ。出なければ、表の最初のフォーカスできる要素へ。
 * 待っている間に、利用者がフォーカスを動かしたら（見出しと body 以外へ）、何もしない。
 * @param {string} id 仕訳の id
 * @param {string} hash 戻り先のアドレス（その画面になってから探す）
 * @param {{ timeout?: number, root?: () => HTMLElement|null }} [opts]
 */
export function focusEntryRowSoon(id, hash, { timeout = 1000, root = () => document.getElementById('main') } = {}) {
  const want = parseHash(hash).path;
  const started = Date.now();
  const put = (el) => {
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: 'nearest' });
  };
  const tick = () => {
    const main = root();
    const here = Boolean(main) && parseHash(location.hash || '#/').path === want && !document.querySelector('dialog[open]');
    if (here) {
      const active = document.activeElement;
      if (active && active !== document.body && active !== main.querySelector('h1')) return;
      const row = main.querySelector(`tr[data-entry-id="${CSS.escape(id)}"]`);
      if (row) return put(row);
    }
    if (Date.now() - started < timeout) {
      setTimeout(tick, 50);
      return undefined;
    }
    const first = here ? firstFocusable(main) : null;
    if (first) put(first);
    return undefined;
  };
  tick();
}
