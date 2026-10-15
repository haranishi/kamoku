// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の「無視」と「戻す」（SC-03、AC-05.7）。無視した行は、通知の「元に戻す」か、一覧での ⌘Z（Mac 以外は Ctrl+Z）で戻せる。
// ⌘Z は、新しく無視した行から順に戻す。文字を打つ欄（input・textarea）にいるときは、欄の標準の取り消しを邪魔しないよう、何もしない。
// 無視は、押したらすぐ次の行へフォーカスを移し、送るのは押した順に1件ずつ。前は応えを待ってから移していたので、続けて押した x が
// 同じ行に届いて同じ行へ送り直し（サーバーは断る）、3回押しても2件しか無視されなかった（2026-10-15 の体験の評価 E2）。

import { shouldIgnoreKey } from '../ime.js';
import { isMac } from '../keys.js';

const NOT_TEXT = new Set(['checkbox', 'radio', 'button', 'submit', 'reset', 'range', 'color', 'file', 'image']);

/** 文字を打つ欄なら真。チェック・ボタン・選択欄は含めない（行の印で x を押した後も、そこで ⌘Z を使えるように）。 */
export function isTextEntry(el) {
  if (!el || typeof el.tagName !== 'string') return false;
  if (el.isContentEditable || el.tagName === 'TEXTAREA') return true;
  return el.tagName === 'INPUT' && !NOT_TEXT.has(String(el.type || 'text').toLowerCase());
}

/** 取り消しのキー（Mac は ⌘Z、それ以外は Ctrl+Z）なら真。Shift つき（やり直し）と Alt つきは含めない。 */
export function isUndoKey(e, mac) {
  if ((e.key !== 'z' && e.key !== 'Z') || e.shiftKey || e.altKey) return false;
  return Boolean(mac ? e.metaKey && !e.ctrlKey : e.ctrlKey && !e.metaKey);
}

/**
 * @param {{ api: any, state: any, toast: any, drop: (id: number) => void, rows: () => object[],
 *           reload: (after: () => void) => void, stale: (err: Error) => void, status: () => string,
 *           ignoredIds: () => number[], ignoredButtons: () => HTMLElement[], focusDraftTab: () => void, focusTop: () => void }} o
 *   drop: その番号の行を、いまの一覧から外す（読み直した後なら、読み直した一覧の行）。reload: 一覧を読み直し、描いた後に after を呼ぶ。
 *   ignoredIds・ignoredButtons: 無視した行の一覧の、行の番号の並びと「戻す」のボタンの並び（同じ順）。
 *   focusTop: 行が1つも残らなかったときの、フォーカスの行き先（見出し）。
 */
export function createIgnoreFlow(o) {
  // 戻せる行（新しい順に後ろ）。通知を閉じる関数も持つ（戻したら、古い「元に戻す」を残さない）。
  const undo = [];
  // 送っている途中の行の番号。同じ行を2回送らない（読み直した一覧の、同じ番号の行でも）。
  const leaving = new Set();
  let chain = Promise.resolve();
  let waiting = 0;

  function forget(id) {
    const i = undo.findIndex((u) => u.id === id);
    if (i < 0) return;
    const [u] = undo.splice(i, 1);
    if (u.dismiss) u.dismiss();
  }

  /** フォーカスの行き先: この行の次の、送っていない行。無ければ前の行。どれも無ければ null。 */
  function nextRow(row) {
    const list = o.rows();
    const i = list.indexOf(row);
    const free = (r) => r !== row && !leaving.has(r.id);
    return list.slice(i + 1).find(free) || list.slice(0, Math.max(i, 0)).reverse().find(free) || null;
  }

  async function send(row) {
    try {
      await o.api.post(`/drafts/${row.id}/ignore`);
    } catch (err) {
      leaving.delete(row.id);
      if (err.code === 'already_confirmed') return o.stale(err);
      const now = o.rows().find((r) => r.id === row.id);
      if (now) {
        now.leave(false);
        now.showError(err.message);
      }
      return undefined;
    }
    leaving.delete(row.id);
    o.drop(row.id);
    o.state.load('status', { force: true }).catch(() => {});
    const entry = { id: row.id, dismiss: null };
    undo.push(entry);
    // 続けて無視しても、通知は1つだけ（新しい行のぶんが前のを置き換える）。積み重なると、表の右下の「無視」などを覆った。
    // 前の行は、⌘Z（新しい順）と「無視した行」から戻せる。
    entry.dismiss = o.toast.success(`「${row.draft.description || '摘要なし'}」を無視しました。`, { action: { label: '元に戻す', onClick: () => restore(row.id) }, ms: 10000, replaceKey: 'inbox-ignore' });
    return undefined;
  }

  /** 送り終えたら、行が残っていなければ読み直す（次のページの行か、空の状態を出すため）。 */
  function drained() {
    if (o.rows().length) return;
    o.reload(() => {
      const first = o.rows()[0];
      if (first) first.focus('cand');
      else o.focusTop();
    });
  }

  function ignore(row) {
    if (o.state.isReadOnly() || leaving.has(row.id)) return;
    leaving.add(row.id);
    row.leave(true);
    const target = nextRow(row);
    if (target) target.focus('cand');
    waiting += 1;
    chain = chain
      .then(() => send(row))
      .catch(() => {})
      .then(() => {
        waiting -= 1;
        if (!waiting) drained();
      });
  }

  /**
   * 未確認に戻す。戻した後のフォーカス: 未確認の一覧なら、戻した行の印。無視した行の一覧（「戻す」）なら、
   * 次の行の「戻す」、無ければ「未確認」のタブ（フォーカスが body に落ちて、場所を見失った）。
   */
  async function restore(id) {
    const at = o.status() === 'ignored' ? o.ignoredIds().indexOf(id) : -1;
    forget(id);
    try {
      await o.api.post(`/drafts/${id}/restore`);
    } catch (err) {
      o.toast.error(err.message);
      return;
    }
    o.toast.success('未確認に戻しました。');
    o.state.load('status', { force: true }).catch(() => {});
    o.reload(() => {
      if (o.status() === 'draft') {
        const row = o.rows().find((r) => r.id === id);
        if (row) row.focus('cand', { nearest: true });
        return;
      }
      if (at < 0) return;
      const next = o.ignoredButtons()[at];
      if (next) next.focus();
      else o.focusDraftTab();
    });
  }

  /** 文書全体のキー。⌘Z（Ctrl+Z）で、最後に無視した行を戻す。 */
  function onKey(e) {
    if (shouldIgnoreKey(e) || !isUndoKey(e, isMac())) return;
    if (document.querySelector('dialog[open]') || isTextEntry(document.activeElement) || !undo.length) return;
    e.preventDefault();
    restore(undo[undo.length - 1].id);
  }

  return { ignore, restore, onKey };
}
