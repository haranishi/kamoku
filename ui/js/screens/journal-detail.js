// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳の詳細（右からの引き出し `#/journal/<id>`）。明細・取引先・メモ・履歴（登録・訂正・取消を時系列に、理由つき）、
// 訂正（e）と取消（v）。取消は理由の入力と、赤系の確認ボタンを要する（SC-05）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { isTypingTarget } from '../keys.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatYen, formatDate } from '../format.js';
import { openDrawer } from '../components/drawer.js';
import { openModal } from '../components/dialog.js';
import { field, button } from '../components/fields.js';
import { money, emptyValue } from '../components/money.js';
import { stateBadge, detailLines, historyList, latestChange } from './journal-detail-parts.js';
import { phrase } from '../components/phrase.js';
import { lockedDetailNote } from './entry-hints.js';

export { stateBadge };

const KIND = { expense: '支出', income: '収入', transfer: '振替', journal: '仕訳の形', opening: '期首残高' };

/**
 * 取消のダイアログ。理由（必須）と赤系の確認ボタン。取り消せたら、新しい仕訳の表現を返す（やめたら null）。
 * @returns {Promise<object|null>}
 */
export function openVoidDialog({ ctx, entry }) {
  return new Promise((resolve) => {
    let current = entry;
    let result = null;
    const reasonIn = h('textarea', { class: 'textarea', rows: 3, maxlength: 200, placeholder: '例: 二重に登録したため' });
    const reasonF = field({ label: '取消の理由', control: reasonIn, required: true, hint: phrase('履歴に残ります。') });
    const summary = h('p');
    const paint = () => {
      summary.replaceChildren(
        `No.${current.no}（${formatDate(current.date)}・${current.description || '摘要なし'}・${formatYen(current.total)}円）を取り消します。`,
        phrase('取り消すと、元に戻せません。取り消した仕訳は帳票の集計から外れ、記録は履歴に残ります。'),
      );
    };
    paint();
    let sending = false;
    const submit = async (c) => {
      if (sending) return;
      const reason = reasonIn.value.trim();
      if (!reason) {
        reasonF.setError('取消の理由を入れてください。');
        reasonIn.focus();
        return;
      }
      reasonF.setError('');
      c.setError('');
      sending = true;
      c.setBusy(true);
      try {
        result = await ctx.api.post(`/entries/${current.id}/void`, { reason, expected_revision: current.revision });
        ctx.toast.success(`No.${current.no} を取り消しました。`);
        c.setBusy(false);
        c.close('ok');
      } catch (err) {
        c.setBusy(false);
        if (err.isStale) {
          // 入力した理由は残し、最新の版を読み直してから、もう一度押してもらう。
          ctx.toast.info('他の場所で更新されました。読み込み直します。');
          try {
            current = await ctx.api.get(`/entries/${current.id}`);
            paint();
            c.setError(current.state === 'voided' ? 'この仕訳は、ほかの場所ですでに取り消されています。' : `最新の版（第${current.revision}版）を読み込みました。内容を確かめて、もう一度「取り消す」を押してください。`);
          } catch (e2) {
            c.setError(e2.message);
          }
        } else if (err.field === 'reason') {
          reasonF.setError(err.message);
          reasonIn.focus();
        } else {
          c.setError(err.message);
        }
      } finally {
        sending = false;
      }
    };
    openModal({
      title: '仕訳を取り消しますか',
      body: h('div', { class: 'stack' }, summary, reasonF.el),
      actions: [
        { label: 'やめる', onClick: (c) => c.close('cancel') },
        // 登録・訂正と同じく、Ctrl/Cmd+Enter で取り消す（理由の欄は複数行なので、Enterだけでは改行になる）。
        { label: '取り消す', variant: 'danger', modEnter: true, onClick: submit },
      ],
      onClose: () => resolve(result),
    });
  });
}

/**
 * 詳細の引き出しを開く。
 * @param {{ ctx: any, id: string, onClose: (how: string) => void, onChanged: () => void, editHash?: () => string }} opts
 *   editHash: 「訂正する」で移る先のアドレス（訂正の後に詳細を閉じたときの戻り先を、アドレスに持たせるため）
 * @returns {{ close: () => void, id: string }}
 */
export function openEntryDrawer(opts) {
  const { ctx, id } = opts;
  let entry = null;
  // 設定の締めた日（locked_until）。締めた日以前の仕訳は、訂正も取消もできない。
  let lockedUntil = null;
  const content = h('div', { class: 'detail' }, h('p', { class: 'muted' }, '読み込んでいます…'));
  const badgeHost = h('div');
  const drawer = openDrawer({ title: '仕訳の詳細', body: content, labelExtra: badgeHost, onClose: opts.onClose });

  // 取り消すは戻せない操作なので、朱の枠のボタンにして、訂正すると見た目を分ける。
  const editBtn = button({ label: '訂正する', iconName: 'edit', kbd: 'e', onClick: () => edit() });
  const voidBtn = button({ label: '取り消す', iconName: 'ban', kbd: 'v', className: 'btn-void', onClick: () => doVoid() });
  const note = h('p', { class: 'field-hint', id: 'detail-note' });

  const isOpening = () => Boolean(entry && entry.meta && entry.meta.kind === 'opening');

  function edit() {
    if (!entry || editBtn.disabled || isOpening()) return;
    const hash = opts.editHash ? opts.editHash() : `#/entry/${entry.id}`;
    drawer.close('edit');
    ctx.navigate(hash);
  }

  async function doVoid() {
    if (!entry || voidBtn.disabled || isOpening()) return;
    const res = await openVoidDialog({ ctx, entry });
    if (res) {
      opts.onChanged();
      await load();
      // 中身を作り直すと、押したボタン（いまは止めてある）からフォーカスが外れる。板の題名へ置き、読み上げを題名から始める。
      if (drawer.el.isConnected && !drawer.el.contains(document.activeElement)) {
        const title = drawer.el.querySelector('h2[tabindex]');
        if (title) title.focus({ preventScroll: true });
      }
    }
  }

  function render() {
    drawer.setTitle(`No.${entry.no} の仕訳`);
    badgeHost.replaceChildren(stateBadge(entry));
    const ro = ctx.state.isReadOnly();
    const voided = entry.state === 'voided';
    const lockedNote = lockedDetailNote(entry.date, lockedUntil);
    editBtn.disabled = ro || voided || Boolean(lockedNote);
    voidBtn.disabled = ro || voided || Boolean(lockedNote);
    note.textContent = ro ? ctx.state.readOnlyReason() : voided ? '取消済みの仕訳は、訂正も取消もできません。' : lockedNote;
    for (const b of [editBtn, voidBtn]) {
      if (note.textContent) b.setAttribute('aria-describedby', 'detail-note');
      else b.removeAttribute('aria-describedby');
    }
    const memo = entry.input && entry.input.memo;
    // 並び: 金額（大きく）と事実 → 最新の訂正 → 操作 → 明細 → 履歴。第2版の訂正と、訂正・取消の操作が、最初の1画面に入るように。
    replace(content,
      h('p', { class: 'detail-total' }, h('span', { class: 'detail-total-label' }, '金額'), money(entry.total)),
      h(
        'dl',
        { class: 'detail-facts' },
        h('div', null, h('dt', null, '日付'), h('dd', null, formatDate(entry.date))),
        h('div', null, h('dt', null, '種別'), h('dd', null, KIND[entry.kind] || entry.kind)),
        h('div', null, h('dt', null, '摘要'), h('dd', null, entry.description || emptyValue())),
        h('div', null, h('dt', null, '取引先'), h('dd', null, entry.partner ? h('span', { class: 'keep' }, entry.partner) : emptyValue())),
        memo ? h('div', null, h('dt', null, 'メモ'), h('dd', null, memo)) : null,
      ),
      latestChange(entry),
      // 期首残高の仕訳は、仕訳の訂正・取消では直さない。設定の「期首残高」から見直す（2026-10-14 20:10 の決定）。
      isOpening()
        ? h('p', { class: 'detail-actions detail-opening' }, icon('info', { size: 16 }), h('span', null, '期首残高は、設定の「期首残高」から見直します。'), h('a', { class: 'tap', href: '#/settings?section=opening', onclick: () => drawer.close('edit') }, '設定の「期首残高」を開く'))
        : h('div', { class: 'detail-actions' }, editBtn, voidBtn),
      isOpening() ? null : note,
      h('section', { class: 'detail-section', 'aria-labelledby': 'lines-title' }, h('h3', { id: 'lines-title' }, '明細'), detailLines(entry.lines)),
      voided ? h('p', { class: 'small muted' }, phrase('取消済みの仕訳は、最後に有効だった版の明細を出しています。帳票の集計には入りません。')) : null,
      h(
        'section',
        { class: 'detail-section', 'aria-labelledby': 'history-title' },
        h('h3', { id: 'history-title' }, '履歴'),
        // 履歴のどの版にも出る「記録のハッシュ」が何かを、初めて見る人に1行で（2026-10-15 の体験の評価 E1）。
        h('p', { class: 'detail-hash-note' }, phrase('「記録のハッシュ」は、記録ごとの印です。設定の「データ」で、台帳が後から書き換えられていないかを確かめるときに使います。')),
        historyList(entry.history || []),
      ),
    );
  }

  async function load() {
    try {
      const [loaded, settings] = await Promise.all([ctx.api.get(`/entries/${id}`), ctx.state.load('settings').catch(() => null)]);
      entry = loaded;
      lockedUntil = (settings && settings.locked_until) || null;
      render();
    } catch (err) {
      replace(content, h('p', { class: 'field-error' }, icon('alert', { size: 16 }), h('span', null, err.message)));
    }
  }

  // 引き出しの中でも e と v を効かせる（文字を打つ欄の外で）。
  drawer.el.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.altKey || e.ctrlKey || e.metaKey || isTypingTarget(e.target)) return;
    if (e.key === 'e') {
      e.preventDefault();
      edit();
    } else if (e.key === 'v') {
      e.preventDefault();
      doVoid();
    }
  });

  load();
  return { id, close: () => drawer.close('cancel'), get el() { return drawer.el; } };
}
