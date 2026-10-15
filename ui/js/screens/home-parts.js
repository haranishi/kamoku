// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ホームのカード（次にやること、数字、口座、経費の上位、台帳の状態、消費税の見込み）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen, shortHash } from '../format.js';
import { daysInMonth } from '../input-parse.js';
import { toast } from '../components/toast.js';
import { money } from '../components/money.js';
import { kbd, shortcutKeys } from '../components/fields.js';
import { phrase } from '../components/phrase.js';
import { pendingShort } from './report-pending.js';

const yen = (n) => money(n);

/** 大きな数字の段落。文字の数を CSS に渡し、桁が多いときだけ文字を小さくして枠に収める（screens.css の .kpi-value）。 */
function bigValue(n) {
  const el = h('p', { class: 'kpi-value' }, yen(n));
  el.style.setProperty('--chars', String(formatYen(n).length));
  return el;
}

/** 「2026年1月1日から9月30日まで」の「9月30日」。日付の書き方を、1つの文の中でそろえる。 */
function dayLabel(iso) {
  const m = /^\d{4}-(\d{2})-(\d{2})$/.exec(String(iso || ''));
  return m ? `${Number(m[1])}月${Number(m[2])}日` : String(iso || '');
}

/**
 * 次にやること（AC-07.1）。未確認があれば、その件数と確定への入口。無ければ次の候補。
 * 月末の前後（最終日の2日前から、翌月の3日まで）なら、帳票の確認をすすめる。
 * 未確認があるときは、右の空きに件数と出金額を大きな数字で置く。
 */
export function nextActions(next, today) {
  const [y, m, d] = today.split('-').map(Number);
  const nearMonthEnd = d >= daysInMonth(y, m) - 2 || d <= 3;
  let body;
  let figures = null;
  if (next.inbox_draft > 0) {
    body = [
      h('p', { class: 'next-text' }, phrase(`未確認の取引が${formatYen(next.inbox_draft)}件あります。科目を決めて、確定しましょう。`)),
      // 未確認が何かと、確定するまで数字に入らないこと（件数は帳簿全体なので「今月の」とは書かない）。
      h('p', { class: 'muted' }, phrase('取り込んだ明細のうち、まだ確定していないものです。確定すると、ホームと帳票の数字に入ります。')),
      h('div', { class: 'next-actions-buttons' }, h('a', { class: 'btn btn--primary', href: '#/inbox' }, icon('inbox', { size: 18 }), '未確認を開く', kbd('g i'))),
    ];
    figures = h(
      'dl',
      { class: 'next-figures' },
      h('div', null, h('dt', null, '未確認'), h('dd', null, h('span', { class: 'money' }, h('span', { class: 'num' }, formatYen(next.inbox_draft)), h('span', { class: 'unit' }, '件')))),
      h('div', null, h('dt', null, 'そのうち出金'), h('dd', null, money(next.inbox_out_amount))),
    );
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
          ? h('a', { class: 'btn btn--primary', href: '#/reports' }, icon('chart', { size: 18 }), '帳票を開く', kbd('g r'))
          : h('a', { class: 'btn btn--primary', href: '#/entry', 'aria-keyshortcuts': shortcutKeys('n') }, icon('plus-square', { size: 18 }), '取引を登録', kbd('n')),
        h('a', { class: 'btn', href: '#/import' }, icon('import', { size: 18 }), '明細を取り込む'),
      ),
    ];
  }
  return h(
    'section',
    { class: ['card', 'next-card', figures && 'has-figures'], 'aria-labelledby': 'next-title' },
    h('div', { class: 'next-main' }, h('h2', { id: 'next-title', class: 'card-title' }, '次にやること'), body),
    figures,
  );
}

/** 前月との差。「前月比 −232,100円」の1行（数字と「円」は折り返しで離れない）。 */
function diffLine(cur, prev) {
  const diff = cur - prev;
  const iconName = diff > 0 ? 'arrow-up' : diff < 0 ? 'arrow-down' : 'minus';
  const text = diff === 0 ? '前月と同じ' : ['前月比 ', h('span', { class: 'money' }, h('span', { class: 'num' }, `${diff > 0 ? '+' : '−'}${formatYen(Math.abs(diff))}`), h('span', { class: 'unit' }, '円'))];
  return h('p', { class: 'kpi-diff' }, icon(iconName, { size: 16 }), h('span', null, text));
}

/** 前の月の実額。「9月は 500,500円（1か月分）」の1行（矢印と差は出さない）。折れるときも「（1か月分）」の途中では折らない。 */
function prevLine(prev, { month, span }) {
  return h('p', { class: 'kpi-diff' }, h('span', null, `${month}は `, money(prev), span ? h('span', { class: 'keep' }, `（${span}）`) : null));
}

/**
 * 数字のカード。利益が負なら「赤字」の文字と符号で示す（AC-07.4）。札は無彩色（朱は訂正・取消・失敗だけに使う）。
 * notes は、補足の行（「売上」「経費」を別の行にして、金額が途中で折れないようにする）。
 * partial は、見ている月が途中の月のときの、前の月の名前と長さ（{ month: '9月', span: '1か月分' }）。途中の月を前の月の
 * 丸1か月と比べると、売上が落ち込んだように見えた（2026-10-15 の体験の評価）ので、差の代わりに前の月の実額を出す。
 * @param {{ label: string, value: number, prev?: number, profit?: boolean, period?: string, notes?: Array<[string, number]>,
 *           partial?: { month: string, span: string }|null }} p
 */
export function kpiCard({ label, value, prev, profit = false, period, notes, partial = null }) {
  const loss = profit && value < 0;
  return h(
    'div',
    { class: ['card', 'kpi', loss && 'kpi--loss'] },
    h('p', { class: 'kpi-label' }, h('span', null, label), loss ? h('span', { class: 'badge badge--muted' }, icon('alert'), '赤字') : null),
    bigValue(value),
    prev === undefined ? null : partial ? prevLine(prev, partial) : diffLine(value, prev),
    notes
      ? h(
          'dl',
          { class: 'kpi-notes' },
          period ? h('div', { class: 'kpi-period' }, h('dt', null, '期間'), h('dd', null, period)) : null,
          notes.map(([k, v]) => h('div', null, h('dt', null, k), h('dd', null, yen(v)))),
        )
      : null,
  );
}

const WALLET_ICON = { cash: 'wallet', bank: 'building', card: 'card', ewallet: 'wallet' };
const WALLET_TYPE = { cash: '現金', bank: '預金', card: 'カード', ewallet: '電子マネー' };

/** 口座の残高。カードは「未払い」と書く。未確認があれば、その分が残高に入っていないことを見出しの下に書く。 */
export function walletsCard(wallets, pending = 0) {
  return h(
    'section',
    { class: 'card home-card wallets-card', 'aria-labelledby': 'wallets-title' },
    h('div', { class: 'card-head' }, h('h2', { id: 'wallets-title', class: 'card-title' }, '口座の残高'), h('a', { href: '#/wallets', class: 'small tap card-link' }, '口座の一覧')),
    pendingShort(pending),
    h(
      'ul',
      { class: 'rows' },
      wallets.items.map((w) => {
        // 名前の下の型は、名前と同じ言葉（「現金」の下に「現金」）なら出さない（口座の画面と同じ決まり）。
        const sub = w.type === 'card' ? 'カード・未払い額' : WALLET_TYPE[w.type] || '';
        return h(
          'li',
          { class: 'row' },
          h('span', { class: 'row-icon' }, icon(WALLET_ICON[w.type] || 'wallet', { size: 18 })),
          h('span', { class: 'row-main' }, h('span', { class: 'keep' }, w.name), sub && sub !== w.name ? h('span', { class: 'row-sub' }, sub) : null),
          h('span', { class: 'row-value' }, yen(w.balance)),
        );
      }),
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
    { class: 'card home-card', 'aria-labelledby': 'top-title' },
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
              h('span', { class: 'rank-name' }, h('a', { class: 'tap keep', href: `#/journal?account=${encodeURIComponent(i.account)}` }, i.name)),
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

/**
 * 台帳の状態: 記録が正常か、記録の件数（AC-09.2 の入口）。末尾のハッシュと写し取りは「くわしく」に畳む
 * （ハッシュは、あとで照合するための控え。ホームの本文に出すと、何の数字か分からなかった）。
 */
export function ledgerCard(ledger) {
  const ok = ledger.verified;
  const manual = h('div', { class: 'hash-manual' });
  return h(
    'section',
    { class: 'card home-card', 'aria-labelledby': 'ledger-title' },
    h(
      'div',
      { class: 'card-head' },
      h('h2', { id: 'ledger-title', class: 'card-title' }, '台帳の状態'),
      ok ? h('span', { class: 'badge badge--ok' }, icon('shield-check'), '正常') : h('span', { class: 'badge badge--danger' }, icon('shield-alert'), '要確認'),
    ),
    ok
      ? h('p', { class: 'ledger-text' }, `記録は正常です（${formatYen(ledger.events)}件）。`)
      : h('p', { class: 'ledger-text' }, `記録の検査で、壊れた所が見つかりました（${formatYen(ledger.events)}件の記録）。設定の「データ」で、最初に壊れた記録を確かめてください。`),
    h(
      'details',
      { class: 'ledger-more' },
      h('summary', { class: 'tap' }, 'くわしく（照合用の控え）'),
      h('p', { class: 'small muted' }, phrase('台帳の最後の記録の印（ハッシュ）です。紙やメモに控えておくと、あとで台帳が書き換えられていないかを照合できます。')),
      h('dl', { class: 'facts' }, h('div', null, h('dt', null, '末尾のハッシュ'), h('dd', null, h('code', { class: 'mono', title: ledger.head_hash }, shortHash(ledger.head_hash))))),
      h('button', { type: 'button', class: 'btn btn--sm', onclick: () => copyHash(ledger.head_hash, manual) }, icon('copy', { size: 16 }), '控えを写し取る'),
      manual,
    ),
  );
}

/** 消費税の見込み（課税事業者のときだけ。AC-07.5）。 */
export function taxCard(tax) {
  const year = String(tax.as_of || '').slice(0, 4);
  return h(
    'section',
    { class: 'card home-card kpi-host', 'aria-labelledby': 'tax-title' },
    h('div', { class: 'card-head' }, h('h2', { id: 'tax-title', class: 'card-title' }, '消費税の見込み'), h('span', { class: 'badge badge--muted' }, '参考')),
    bigValue(tax.estimate),
    h(
      'dl',
      { class: 'totals' },
      h('div', null, h('dt', null, '売上税額'), h('dd', null, yen(tax.sales_tax))),
      h('div', null, h('dt', null, '仕入税額'), h('dd', null, yen(tax.purchase_tax))),
    ),
    h('p', { class: 'kpi-note' }, phrase(`${year}年1月1日から${dayLabel(tax.as_of)}までの集計です。参考値で、簡易課税と特例は反映していません。`)),
  );
}
