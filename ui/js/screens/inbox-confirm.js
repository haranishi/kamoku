// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認（SC-03）の確定の流れ: 保存待ち → 確認 → 2,000件ずつ送る → 結果を画面に合わせる。
// 最初の待ちの前にロックを取り、確認を二重に開かない（Ctrl/Cmd+Enter の押し直しで、確認が重なって開いた）。
// 保存待ちから、表・絞り込み・ページ送りを操作できなくする（inert）。待つ間に変更を増やすと、保存より先に確定されるため。
// 通信の失敗でも一覧を読み直す（応えが届かなくても、サーバーでは確定済みのことがある）。行ごとの理由は、読み直した行に出す。

import { confirmInChunks, confirmOutcome, confirmedNos } from '../candidates.js';
import { confirmCandidates } from './inbox-bar.js';

/**
 * @param {{ api: any, state: any, toast: any, candidates: object, bar: object, root: HTMLElement,
 *           frozen: HTMLElement[], busyEl: HTMLElement, ready: () => boolean, rows: () => object[],
 *           rowOf: (el: Element) => object|undefined, load: () => Promise<void>, paintBar: () => void, disposed: () => boolean }} o
 *   frozen: 保存待ちから操作を止める要素。busyEl: 同じ間に aria-busy を立てる要素（一覧）。
 */
export function createConfirmFlow(o) {
  let locked = false;
  let sending = null;

  function freeze(on) {
    for (const el of o.frozen) el.inert = on;
    if (on) o.busyEl.setAttribute('aria-busy', 'true');
    else o.busyEl.removeAttribute('aria-busy');
  }

  /** いまフォーカスのある行の番号と列（確定の後に、同じ行へ戻すため）。行の外なら id は null。 */
  function focusSpot() {
    const el = document.activeElement;
    const row = el && el.closest ? o.rowOf(el) : null;
    const td = row && el.closest('td');
    return { id: row ? row.id : null, col: td ? td.dataset.col : null };
  }

  /**
   * 確定の後のフォーカス: 確定の前にいた行が残っていれば、その行の同じ列 → その行が確定して消えたなら、残った一覧の
   * 先頭の行の科目 → 行の外にいたなら、確定のボタン（まだ確定する行があるとき）か先頭の行の科目 → 見出し（何も残っていない）。
   * 前は「同じ番目の行」へ戻していたので、20件を確定すると、残った一覧の途中の行へ飛び、作業していた場所が画面の外に出た。
   */
  function restoreFocus(spot) {
    const rows = o.rows();
    const same = spot.id === null ? null : rows.find((r) => r.id === spot.id);
    if (same) same.focus(spot.col || 'account');
    else if (spot.id === null && !o.bar.confirmButton.disabled) o.bar.confirmButton.focus();
    else if (rows.length) rows[0].focus('account', { nearest: true });
    else o.root.querySelector('h1').focus();
  }

  async function settle(res) {
    const out = confirmOutcome(res);
    out.unmark.forEach((id) => o.candidates.remove(id));
    if (res.confirmed.length) {
      o.toast.success(`${res.confirmed.length}件を確定しました（${confirmedNos(res.confirmed.map((c) => c.no))}）。`, { action: { label: '仕訳帳を見る', href: '#/journal' }, ms: 10000 });
    }
    if (out.fixable.length) o.toast.error(`${out.fixable.length}件は確定できませんでした。行の下の理由を確かめて、直してからもう一度確定してください。`);
    if (out.gone.length) o.toast.info(`${out.gone.length}件は、ほかの場所で確定済みか、見つかりませんでした。確定する行から外しました。`);
    const err = res.error;
    if (err && (err.isStale || err.code === 'already_confirmed')) o.toast.info('他の場所で更新されました。読み込み直します。', { detail: err.message });
    else if (err) o.toast.error(`${err.message} 一覧を読み込み直して、確定できた分を確かめます。`);
    o.state.load('status', { force: true }).catch(() => {});
    await o.load();
    for (const f of out.fixable) {
      const r = o.rows().find((x) => x.id === f.id);
      if (r) r.showError(f.error.message, f.error.field);
    }
  }

  /** 確定する（Ctrl/Cmd+Enter と、下の帯の「確定する」）。 */
  async function run() {
    if (locked || !o.ready()) return undefined;
    if (o.state.isReadOnly()) return o.paintBar();
    const active = document.activeElement;
    const row = active ? o.rowOf(active) : null;
    if (row) row.commit(active.closest('td') ? active.closest('td').dataset.col : null);
    if (!o.candidates.size) {
      o.bar.update(o.candidates.summary(), '確定する行がありません。行で Enter を押して、確定する行にしてください。', '確定する行がありません。左の印を押して、確定する行を選んでください。');
      // 帯の文は、最初から同じ文なので、押しても変わらず、読み上げでは何も起きなかった（体験の評価 第3回 E4）。通知（読み上げの領域）でも返す。
      o.toast.info('確定する行がありません。行で Enter を押すと、確定する行になります。');
      return undefined;
    }
    locked = true;
    const spot = focusSpot();
    try {
      freeze(true);
      await Promise.all(o.rows().map((r) => r.flush()));
      if (o.disposed() || !(await confirmCandidates(o.candidates.summary())) || o.disposed()) return undefined;
      sending = o.candidates.summary();
      o.paintBar();
      // 2,000件を超えたら、2,000件ずつに分けて順に送る（サーバーの上限。candidates.js）。
      const res = await confirmInChunks(o.candidates.ids(), (ids) => o.api.post('/drafts/confirm', { ids }));
      if (!o.disposed()) await settle(res);
    } finally {
      locked = false;
      sending = null;
      freeze(false);
      if (!o.disposed()) {
        o.paintBar();
        restoreFocus(spot);
      }
    }
    return undefined;
  }

  return {
    run,
    /** 送っている間の、確定の件数と合計（下のバーに固定して見せる）。送っていなければ null。 */
    get sending() {
      return sending;
    },
  };
}
