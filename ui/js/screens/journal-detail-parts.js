// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳の詳細（journal-detail.js）の見せ方の部品: 状態の札、明細（広い幅は表、狭い幅は借方と貸方の2列）、履歴、最新の訂正。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatDateTime, shortHash } from '../format.js';
import { TAX_LABELS } from '../account-defaults.js';
import { money, emptyValue } from '../components/money.js';
import { previewTable } from './entry-preview.js';
import { lineMemo } from './entry-hints.js';

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

/**
 * 状態の札（文字と記号。色だけに頼らない）。訂正も取消も朱の淡い色で、文字と記号で見分ける。
 * quiet が真なら、有効な行（訂正なし）には何も出さない（一覧で全行に「有効」が並び、目が止まらなかった）。
 */
export function stateBadge(e, { quiet = false } = {}) {
  if (e.state === 'voided') return h('span', { class: 'badge badge--danger' }, icon('ban'), '取消済み');
  if (e.revision > 1) return h('span', { class: 'badge badge--danger' }, icon('edit'), `訂正 第${e.revision}版`);
  return quiet ? null : h('span', { class: 'state-plain' }, '有効');
}

const taxText = (l) => (l.tax_code ? TAX_LABELS[l.tax_code] || l.tax_code : '対象外');

/** 広い幅の明細の表。メモは科目の下、税額は税区分の下に畳み、4列で引き出しの幅に収める（メモが右の枠に接していた）。 */
function linesTable(lines) {
  return h(
    'div',
    { class: 'table-wrap detail-lines-wrap' },
    h(
      'table',
      { class: 'table detail-lines' },
      h('caption', null, '明細'),
      h('thead', null, h('tr', null, h('th', { scope: 'col' }, '貸借'), h('th', { scope: 'col' }, '科目・メモ'), h('th', { scope: 'col' }, '税区分・税額'), h('th', { scope: 'col', class: 'num' }, '金額'))),
      h(
        'tbody',
        null,
        lines.map((l) =>
          h(
            'tr',
            null,
            h('td', { class: 'detail-side' }, l.side === 'debit' ? '借方' : '貸方'),
            h('td', null, h('span', { class: 'keep detail-account' }, l.account_name || l.account), lineMemo(l) ? h('span', { class: 'sub' }, lineMemo(l)) : null),
            h('td', null, h('span', { class: 'keep' }, taxText(l)), h('span', { class: 'sub' }, '税額 ', l.tax_amount ? money(l.tax_amount) : emptyValue())),
            h('td', { class: 'num' }, money(l.amount)),
          ),
        ),
      ),
    ),
  );
}

/** 狭い幅の明細: 借方と貸方の2列のかたまり。1明細を5〜6段の縦並びにすると、どちらにいくら入るか追えなかった。 */
function linesSides(lines) {
  const side = (key, title) =>
    h(
      'div',
      { class: 'detail-side-col' },
      h('p', { class: 'detail-side-title' }, title),
      h(
        'ul',
        { class: 'detail-side-list' },
        lines
          .filter((l) => l.side === key)
          .map((l) =>
            h(
              'li',
              null,
              h('span', { class: 'keep detail-account' }, l.account_name || l.account),
              h('span', { class: 'detail-amount' }, money(l.amount)),
              h('span', { class: 'sub' }, taxText(l), l.tax_amount ? ['・税', money(l.tax_amount)] : null),
              lineMemo(l) ? h('span', { class: 'sub' }, lineMemo(l)) : null,
            ),
          ),
      ),
    );
  return h('div', { class: 'detail-sides', role: 'group', 'aria-label': '明細（借方と貸方）' }, side('debit', '借方'), side('credit', '貸方'));
}

/** 明細。広い幅の表と、狭い幅の2列を両方作り、CSS で片方だけを見せる（screens-journal-detail.css）。 */
export function detailLines(lines) {
  return [linesTable(lines), linesSides(lines)];
}

/** 履歴（登録・訂正・取消を時系列に、理由つき）。 */
export function historyList(history) {
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
 * 最新の訂正・取消の要約（第2版以降だけ）。履歴は下にあるので、最初の1画面で、何が・なぜ変わったかが見えるようにする。
 * @returns {HTMLElement|null}
 */
export function latestChange(entry) {
  const history = entry.history || [];
  const last = history.at(-1);
  if (!last || history.length < 2) return null;
  const [label, iconName] = EVENT[last.kind] || [last.kind, 'clock'];
  return h(
    'p',
    { class: 'detail-latest' },
    icon(iconName, { size: 16 }),
    h('span', null, h('strong', null, `${label} 第${last.revision}版`), `（${formatDateTime(last.recorded_at)}）`, last.reason ? `　理由: ${last.reason}` : null),
  );
}
