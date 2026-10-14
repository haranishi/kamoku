// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ホームのカード（次にやること、数字、口座、経費の上位、台帳の状態、消費税の見込み）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen, describeDiff, shortHash, formatDate } from '../format.js';
import { daysInMonth } from '../input-parse.js';
import { toast } from '../components/toast.js';

const yen = (n) => [h('span', { class: 'num' }, formatYen(n)), h('span', { class: 'unit' }, '円')];

/**
 * 次にやること（AC-07.1）。未確認があれば、その件数と確定への入口。無ければ次の候補。
 * 月末の前後（最終日の2日前から、翌月の3日まで）なら、帳票の確認をすすめる。
 */
export function nextActions(next, today) {
  const [y, m, d] = today.split('-').map(Number);
  const nearMonthEnd = d >= daysInMonth(y, m) - 2 || d <= 3;
  let body;
  if (next.inbox_draft > 0) {
    body = [
      h('p', { class: 'next-text' }, `未確認の取引が ${next.inbox_draft}件あります（出金 ${formatYen(next.inbox_out_amount)}円）。`),
      h('div', { class: 'next-actions-buttons' }, h('a', { class: 'btn btn--primary', href: '#/inbox' }, icon('inbox', { size: 18 }), '未確認を開く', h('kbd', null, 'g i'))),
    ];
  } else {
    body = [
      h('p', { class: 'next-text' }, '未確認はありません。'),
      nearMonthEnd
        ? h('p', { class: 'muted' }, '月の変わり目です。今月の数字を帳票で確かめましょう。')
        : h('p', { class: 'muted' }, '明細を取り込むか、取引を登録しましょう。'),
      h(
        'div',
        { class: 'next-actions-buttons' },
        nearMonthEnd
          ? h('a', { class: 'btn btn--primary', href: '#/reports' }, icon('chart', { size: 18 }), '帳票を開く', h('kbd', null, 'g r'))
          : h('a', { class: 'btn btn--primary', href: '#/entry' }, icon('plus-square', { size: 18 }), '取引を登録', h('kbd', null, 'n')),
        h('a', { class: 'btn', href: '#/import' }, icon('import', { size: 18 }), '明細を取り込む'),
      ),
    ];
  }
  return h('section', { class: 'card next-card', 'aria-labelledby': 'next-title' }, h('h2', { id: 'next-title', class: 'card-title' }, '次にやること'), body);
}

function diffLine(cur, prev) {
  const d = describeDiff(cur, prev);
  const iconName = d.dir === 'up' ? 'arrow-up' : d.dir === 'down' ? 'arrow-down' : 'minus';
  return h('p', { class: 'kpi-diff' }, icon(iconName, { size: 16 }), h('span', null, d.text));
}

/**
 * 数字のカード。利益が負なら「赤字」の文字と符号で示す（AC-07.4）。赤（朱）は使わない。
 * @param {{ label: string, value: number, prev?: number, profit?: boolean, note?: string }} p
 */
export function kpiCard({ label, value, prev, profit = false, note }) {
  const loss = profit && value < 0;
  return h(
    'div',
    { class: ['card', 'kpi', loss && 'kpi--loss'] },
    h('p', { class: 'kpi-label' }, label, loss ? h('span', { class: 'badge badge--warn' }, icon('alert'), '赤字') : null),
    h('p', { class: 'kpi-value' }, yen(value)),
    prev !== undefined ? diffLine(value, prev) : null,
    note ? h('p', { class: 'kpi-note' }, note) : null,
  );
}

const WALLET_ICON = { cash: 'wallet', bank: 'building', card: 'card', ewallet: 'wallet' };
const WALLET_TYPE = { cash: '現金', bank: '預金', card: 'カード', ewallet: '電子マネー' };

/** 口座の残高。カードは「未払い」と書く。 */
export function walletsCard(wallets) {
  return h(
    'section',
    { class: 'card', 'aria-labelledby': 'wallets-title' },
    h('div', { class: 'card-head' }, h('h2', { id: 'wallets-title', class: 'card-title' }, '口座の残高'), h('a', { href: '#/wallets', class: 'small' }, '口座の一覧')),
    h(
      'ul',
      { class: 'rows' },
      wallets.items.map((w) =>
        h(
          'li',
          { class: 'row' },
          h('span', { class: 'row-icon' }, icon(WALLET_ICON[w.type] || 'wallet', { size: 18 })),
          h('span', { class: 'row-main' }, h('span', null, w.name), h('span', { class: 'row-sub' }, w.type === 'card' ? 'カード・未払い額' : WALLET_TYPE[w.type] || '')),
          h('span', { class: 'row-value' }, yen(w.balance)),
        ),
      ),
    ),
    h(
      'dl',
      { class: 'totals' },
      h('div', null, h('dt', null, '現金と預金の合計'), h('dd', null, yen(wallets.cash_total))),
      h('div', null, h('dt', null, 'カードの未払いの合計'), h('dd', null, yen(wallets.owed_total))),
    ),
  );
}

/** 経費の上位5科目。棒は飾りで、金額は文字で出す。 */
export function topExpensesCard(items, monthLabel) {
  const max = Math.max(1, ...items.map((i) => i.amount));
  return h(
    'section',
    { class: 'card', 'aria-labelledby': 'top-title' },
    h('div', { class: 'card-head' }, h('h2', { id: 'top-title', class: 'card-title' }, '経費の上位5科目'), h('span', { class: 'small muted' }, monthLabel)),
    items.length
      ? h(
          'ol',
          { class: 'rank' },
          items.map((i) => {
            const bar = h('span', { class: 'rank-bar', 'aria-hidden': 'true' });
            bar.style.setProperty('--w', `${Math.max(2, Math.round((i.amount / max) * 100))}%`);
            return h(
              'li',
              { class: 'rank-item' },
              h('span', { class: 'rank-name' }, h('a', { href: `#/journal?account=${encodeURIComponent(i.account)}` }, i.name)),
              h('span', { class: 'rank-value' }, yen(i.amount)),
              bar,
            );
          }),
        )
      : h('p', { class: 'muted' }, 'この月の経費は、まだありません。'),
  );
}

async function copyHash(hash, host) {
  try {
    await navigator.clipboard.writeText(hash);
    toast.success('末尾のハッシュを写し取りました。紙やメモに控えておくと、あとで台帳と照合できます。');
  } catch {
    // 写せない環境では、全文を選んだ状態で見せ、手で写してもらう。
    const field = h('input', { class: 'input mono', readonly: true, value: hash, 'aria-label': '末尾のハッシュ（全文）' });
    host.replaceChildren(field, h('p', { class: 'field-hint' }, '自動で写せませんでした。選んである文字を Ctrl+C（Macは ⌘+C）で写してください。'));
    field.focus();
    field.select();
  }
}

/** 台帳の状態: 整合性、記録の件数、末尾のハッシュの先頭8桁、写し取る（AC-09.2 の入口）。 */
export function ledgerCard(ledger) {
  const ok = ledger.verified;
  const manual = h('div', { class: 'hash-manual' });
  return h(
    'section',
    { class: 'card', 'aria-labelledby': 'ledger-title' },
    h(
      'div',
      { class: 'card-head' },
      h('h2', { id: 'ledger-title', class: 'card-title' }, '台帳の状態'),
      ok
        ? h('span', { class: 'badge badge--ok' }, icon('shield-check'), '整合性 OK')
        : h('span', { class: 'badge badge--danger' }, icon('shield-alert'), '要確認'),
    ),
    h(
      'dl',
      { class: 'facts' },
      h('div', null, h('dt', null, '記録の件数'), h('dd', null, h('span', { class: 'num' }, formatYen(ledger.events)), '件')),
      h('div', null, h('dt', null, '末尾のハッシュ'), h('dd', null, h('code', { class: 'mono', title: ledger.head_hash }, shortHash(ledger.head_hash)))),
    ),
    h('button', { type: 'button', class: 'btn btn--sm', onclick: () => copyHash(ledger.head_hash, manual) }, icon('copy', { size: 16 }), '写し取る'),
    manual,
    ok ? null : h('p', { class: 'small' }, '検証で不正が見つかりました。設定の「データ」で、最初に壊れた記録を確かめてください。'),
  );
}

/** 消費税の見込み（課税事業者のときだけ。AC-07.5）。 */
export function taxCard(tax) {
  return h(
    'section',
    { class: 'card', 'aria-labelledby': 'tax-title' },
    h('div', { class: 'card-head' }, h('h2', { id: 'tax-title', class: 'card-title' }, '消費税の見込み'), h('span', { class: 'badge badge--muted' }, '参考')),
    h('p', { class: 'kpi-value' }, yen(tax.estimate)),
    h(
      'dl',
      { class: 'totals' },
      h('div', null, h('dt', null, '売上税額'), h('dd', null, yen(tax.sales_tax))),
      h('div', null, h('dt', null, '仕入税額'), h('dd', null, yen(tax.purchase_tax))),
    ),
    h('p', { class: 'kpi-note' }, `1月1日から${formatDate(tax.as_of)}までの集計です。参考値で、簡易課税と特例は反映していません。`),
  );
}
