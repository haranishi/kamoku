// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳の詳細（右からの引き出し `#/journal/<id>`）。明細・取引先・メモ・履歴（登録・訂正・取消を時系列に、理由つき）、
// 訂正（e）と取消（v）。取消は理由の入力と、赤系の確認ボタンを要する（SC-05）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { isTypingTarget } from '../keys.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatYen, formatDate, formatDateTime, shortHash } from '../format.js';
import { TAX_LABELS } from '../account-defaults.js';
import { openDrawer } from '../components/drawer.js';
import { openModal } from '../components/dialog.js';
import { field, button } from '../components/fields.js';
import { previewTable } from './entry-preview.js';

const KIND = { expense: '支出', income: '収入', transfer: '振替', journal: '仕訳の形', opening: '期首残高' };
// 履歴の種類の名前は仕様で決まっていない（spec-issues.md の1）。台帳の名前と、別の書き方の両方を読む。
const EVENT = {
  posted: ['登録', 'plus'],
  create: ['登録', 'plus'],
  created: ['登録', 'plus'],
  revised: ['訂正', 'edit'],
  correct: ['訂正', 'edit'],
  corrected: ['訂正', 'edit'],
  voided: ['取消', 'ban'],
  void: ['取消', 'ban'],
};

/** 状態の札（文字と記号。色だけに頼らない）。 */
export function stateBadge(e) {
  if (e.state === 'voided') return h('span', { class: 'badge badge--danger' }, icon('ban'), '取消済み');
  if (e.revision > 1) return h('span', { class: 'badge badge--warn' }, icon('edit'), `訂正 第${e.revision}版`);
  // 有効な行は数が多いので、札にせず文字だけにする（取消済みと訂正が目に入りやすいように）。
  return h('span', { class: 'state-plain' }, '有効');
}

function linesTable(lines) {
  return h(
    'div',
    { class: 'table-wrap' },
    h(
      'table',
      { class: 'table table--stack detail-lines' },
      h('caption', null, '明細'),
      h('thead', null, h('tr', null, ['借方・貸方', '科目', '金額', '税区分', '税額', 'メモ'].map((t, i) => h('th', { scope: 'col', class: i === 2 || i === 4 ? 'num' : null }, t)))),
      h(
        'tbody',
        null,
        lines.map((l) =>
          h(
            'tr',
            null,
            h('td', { 'data-label': '借方・貸方' }, l.side === 'debit' ? '借方' : '貸方'),
            h('td', { 'data-label': '科目' }, l.account_name || l.account),
            h('td', { class: 'num', 'data-label': '金額' }, formatYen(l.amount)),
            h('td', { 'data-label': '税区分' }, l.tax_code ? TAX_LABELS[l.tax_code] || l.tax_code : '対象外'),
            h('td', { class: 'num', 'data-label': '税額' }, l.tax_amount ? formatYen(l.tax_amount) : '—'),
            h('td', { class: 'wrap', 'data-label': 'メモ' }, l.memo || null),
          ),
        ),
      ),
    ),
  );
}

function historyList(history) {
  return h(
    'ol',
    { class: 'timeline' },
    history.map((ev) => {
      const [label, iconName] = EVENT[ev.kind] || [ev.kind, 'clock'];
      return h(
        'li',
        { class: ['timeline-item', `timeline-item--${iconName}`] },
        h('span', { class: 'timeline-icon' }, icon(iconName, { size: 16 })),
        h(
          'div',
          { class: 'timeline-body' },
          h('p', { class: 'timeline-title' }, h('strong', null, label), ` 第${ev.revision}版`, h('span', { class: 'muted small' }, `　${formatDateTime(ev.recorded_at)}`)),
          ev.reason ? h('p', null, `理由: ${ev.reason}`) : null,
          h('p', { class: 'muted small' }, '記録のハッシュ ', h('code', { class: 'mono', title: ev.hash }, shortHash(ev.hash)), `（${ev.seq}番目の記録）`),
          ev.lines ? h('details', { class: 'timeline-lines' }, h('summary', null, 'この版の明細'), previewTable(ev.lines, `第${ev.revision}版の明細`)) : null,
        ),
      );
    }),
  );
}

/**
 * 取消のダイアログ。理由（必須）と赤系の確認ボタン。取り消せたら、新しい仕訳の表現を返す（やめたら null）。
 * @returns {Promise<object|null>}
 */
export function openVoidDialog({ ctx, entry }) {
  return new Promise((resolve) => {
    let current = entry;
    let result = null;
    const reasonIn = h('textarea', { class: 'textarea', rows: 3, maxlength: 200, placeholder: '例: 二重に登録したため' });
    const reasonF = field({ label: '取消の理由', control: reasonIn, required: true, hint: '履歴に残ります。' });
    const summary = h('p');
    const paint = () => {
      summary.textContent = `No.${current.no}（${formatDate(current.date)}・${current.description || '摘要なし'}・${formatYen(current.total)}円）を取り消します。取り消した仕訳は帳票の集計から外れ、記録は履歴に残ります。`;
    };
    paint();
    const submit = async (c) => {
      const reason = reasonIn.value.trim();
      if (!reason) {
        reasonF.setError('取消の理由を入れてください。');
        reasonIn.focus();
        return;
      }
      reasonF.setError('');
      c.setError('');
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
      }
    };
    openModal({
      title: '仕訳を取り消しますか',
      body: h('div', { class: 'stack' }, summary, reasonF.el),
      actions: [
        { label: 'やめる', onClick: (c) => c.close('cancel') },
        { label: '取り消す', variant: 'danger', onClick: submit },
      ],
      onClose: () => resolve(result),
    });
  });
}

/**
 * 詳細の引き出しを開く。
 * @param {{ ctx: any, id: string, onClose: () => void, onChanged: () => void }} opts
 * @returns {{ close: () => void, id: string }}
 */
export function openEntryDrawer(opts) {
  const { ctx, id } = opts;
  let entry = null;
  const content = h('div', { class: 'detail' }, h('p', { class: 'muted' }, '読み込んでいます…'));
  const badgeHost = h('div');
  const drawer = openDrawer({ title: '仕訳の詳細', body: content, labelExtra: badgeHost, onClose: opts.onClose });

  const editBtn = button({ label: '訂正する', iconName: 'edit', kbd: 'e', onClick: () => edit() });
  const voidBtn = button({ label: '取り消す', iconName: 'ban', kbd: 'v', onClick: () => doVoid() });
  const note = h('p', { class: 'field-hint', id: 'detail-note' });

  const isOpening = () => Boolean(entry && entry.meta && entry.meta.kind === 'opening');

  function edit() {
    if (!entry || editBtn.disabled || isOpening()) return;
    drawer.close('edit');
    ctx.navigate(`#/entry/${entry.id}`);
  }

  async function doVoid() {
    if (!entry || voidBtn.disabled || isOpening()) return;
    const res = await openVoidDialog({ ctx, entry });
    if (res) {
      opts.onChanged();
      load();
    }
  }

  function render() {
    drawer.setTitle(`No.${entry.no} の仕訳`);
    badgeHost.replaceChildren(stateBadge(entry));
    const ro = ctx.state.isReadOnly();
    const voided = entry.state === 'voided';
    editBtn.disabled = ro || voided;
    voidBtn.disabled = ro || voided;
    note.textContent = ro ? ctx.state.readOnlyReason() : voided ? '取消済みの仕訳は、訂正も取消もできません。' : '';
    for (const b of [editBtn, voidBtn]) {
      if (note.textContent) b.setAttribute('aria-describedby', 'detail-note');
      else b.removeAttribute('aria-describedby');
    }
    const memo = entry.input && entry.input.memo;
    replace(content, 
      h(
        'dl',
        { class: 'detail-facts' },
        h('div', null, h('dt', null, '日付'), h('dd', null, formatDate(entry.date))),
        h('div', null, h('dt', null, '種別'), h('dd', null, KIND[entry.kind] || entry.kind)),
        h('div', null, h('dt', null, '摘要'), h('dd', null, entry.description || '（なし）')),
        h('div', null, h('dt', null, '取引先'), h('dd', null, entry.partner || '（なし）')),
        h('div', null, h('dt', null, '金額'), h('dd', { class: 'num' }, `${formatYen(entry.total)}円`)),
        memo ? h('div', null, h('dt', null, 'メモ'), h('dd', null, memo)) : null,
      ),
      linesTable(entry.lines),
      voided ? h('p', { class: 'small muted' }, '取消済みの仕訳は、最後に有効だった版の明細を出しています。帳票の集計には入りません。') : null,
      h('section', { class: 'detail-section', 'aria-labelledby': 'history-title' }, h('h3', { id: 'history-title' }, '履歴'), historyList(entry.history || [])),
      // 期首残高の仕訳は、仕訳の訂正・取消では直さない。設定の「期首残高」から見直す（2026-10-14 20:10 の決定）。
      isOpening()
        ? h('p', { class: 'detail-actions detail-opening' }, icon('info', { size: 16 }), h('span', null, '期首残高は、設定の「期首残高」から見直します。'), h('a', { href: '#/settings?section=opening', onclick: () => drawer.close('edit') }, '設定の「期首残高」を開く'))
        : h('div', { class: 'detail-actions' }, editBtn, voidBtn),
      isOpening() ? null : note,
    );
  }

  async function load() {
    try {
      entry = await ctx.api.get(`/entries/${id}`);
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
