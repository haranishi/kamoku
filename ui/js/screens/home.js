// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-01 ホーム `#/`。一番上は「次にやること」（AC-07.1）。数字は1回の通信（/reports/dashboard）で受け取る。
// 月はアドレスの ?month= に持つ（戻る操作で前の月に戻れる）。今月のときは書かない。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatMonth, formatYen } from '../format.js';
import { todayIso, shiftMonth } from '../input-parse.js';
import { createChart } from '../components/chart.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { banner } from '../components/banner.js';
import { iconButton } from '../components/fields.js';
import { nextActions, kpiCard, walletsCard, topExpensesCard, ledgerCard, taxCard } from './home-parts.js';

const isMonth = (s) => /^\d{4}-(0[1-9]|1[0-2])$/.test(s || '');

function skeletonView() {
  return h(
    'div',
    { class: 'home' },
    skeletonBlock({ lines: 2, label: 'ホームの数字を読み込んでいます' }),
    h('div', { class: 'kpis' }, [0, 1, 2, 3].map(() => skeletonBlock({ lines: 2, widths: ['50%', '80%'] }))),
    skeletonBlock({ lines: 5 }),
  );
}

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const today = todayIso();
  const thisMonth = today.slice(0, 7);
  let month = isMonth(ctx.query.month) ? ctx.query.month : thisMonth;
  let chart = null;
  let seq = 0;
  let disposed = false;

  const select = h('select', { class: 'select month-select', 'aria-label': '表示する月' });
  const prevBtn = iconButton({ iconName: 'chevron-left', label: '前の月', onClick: () => go(shiftMonth(month, -1)) });
  const nextBtn = iconButton({ iconName: 'chevron-right', label: '次の月', onClick: () => go(shiftMonth(month, 1)) });
  select.addEventListener('change', () => go(select.value));
  const body = h('div', { class: 'home' });

  root.append(
    h(
      'div',
      { class: 'page-head' },
      h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, 'ホーム')),
      h(
        'div',
        { class: 'page-head-actions' },
        h('div', { class: 'month-picker' }, prevBtn, select, nextBtn),
        // スマホの幅では、下のバーの「登録」と同じなので隠す（見出しの行を1行に収めるため）。
        h('a', { class: 'btn btn--primary hide-mobile', href: '#/entry' }, icon('plus-square', { size: 18 }), '取引を登録', h('kbd', null, 'n')),
      ),
    ),
    body,
  );

  function go(m) {
    if (m > thisMonth) return;
    ctx.navigate(ctx.router.buildHash('/', { month: m === thisMonth ? null : m }));
  }

  function fillMonths() {
    select.replaceChildren();
    const months = [];
    for (let i = 0; i < 24; i += 1) months.push(shiftMonth(thisMonth, -i));
    if (!months.includes(month)) months.push(month);
    for (const m of months) select.append(h('option', { value: m }, formatMonth(m)));
    select.value = month;
    nextBtn.disabled = month >= thisMonth;
  }

  function render(dash, status) {
    if (chart) chart.destroy();
    chart = null;
    const mLabel = `${Number(month.slice(5, 7))}月`;
    const parts = [nextActions(dash.next_actions, today)];
    if (status && status.ledger && status.ledger.entries === 0) {
      parts.push(
        h(
          'div',
          { class: 'card' },
          emptyState({
            iconName: 'plus-square',
            title: '最初の取引を登録しましょう',
            body: '取引を1件登録すると、ここに今月の売上・経費・利益と、口座の残高が出ます。',
            actions: [
              { label: '取引を登録', href: '#/entry', primary: true, kbd: 'n' },
              { label: '明細を取り込む', href: '#/import' },
            ],
          }),
        ),
      );
      replace(body, ...parts);
      return;
    }
    const ms = dash.month_summary;
    parts.push(
      h(
        'section',
        { class: 'kpis', 'aria-label': `${formatMonth(month)}の数字` },
        kpiCard({ label: `${mLabel}の売上`, value: ms.revenue, prev: ms.prev.revenue }),
        kpiCard({ label: `${mLabel}の経費`, value: ms.expense, prev: ms.prev.expense }),
        kpiCard({ label: `${mLabel}の利益`, value: ms.profit, prev: ms.prev.profit, profit: true }),
        kpiCard({
          label: `${dash.fiscal_year}年の利益`,
          value: dash.ytd.profit,
          profit: true,
          note: `1月〜${mLabel}。売上 ${formatYen(dash.ytd.revenue)}円、経費 ${formatYen(dash.ytd.expense)}円`,
        }),
      ),
    );
    // 未来の月は描かない（0円の月が並ぶと、急に落ち込んだように見えるため）。
    const year = String(dash.fiscal_year);
    const until = year < thisMonth.slice(0, 4) ? `${year}-12` : thisMonth;
    chart = createChart({ series: dash.series, until, highlight: month, label: `${year}年の推移` });
    parts.push(
      h(
        'section',
        { class: 'card chart-card', 'aria-labelledby': 'chart-title' },
        h('div', { class: 'card-head' }, h('h2', { id: 'chart-title', class: 'card-title' }, `${year}年の推移`)),
        chart.el,
      ),
    );
    parts.push(
      h(
        'div',
        { class: 'home-grid' },
        walletsCard(dash.wallets),
        topExpensesCard(dash.top_expenses, formatMonth(month)),
        // 3列目に台帳と消費税を縦に重ね、消費税のカードだけが次の行に落ちないようにする。
        h('div', { class: 'home-stack' }, ledgerCard(dash.ledger), dash.tax && dash.tax.applicable ? taxCard(dash.tax) : null),
      ),
    );
    replace(body, ...parts);
  }

  async function load() {
    const my = ++seq;
    fillMonths();
    body.replaceChildren();
    const stop = delayedSkeleton(body, skeletonView);
    try {
      const [dash, status] = await Promise.all([ctx.api.get('/reports/dashboard', { month }), ctx.state.load('status', { force: true })]);
      stop();
      if (disposed || my !== seq) return;
      render(dash, status);
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      replace(body, 
        banner({
          kind: 'danger',
          title: 'ホームの数字を読み込めませんでした',
          body: err.message,
          actions: h('div', null, h('button', { type: 'button', class: 'btn btn--sm', onclick: load }, 'もう一度読み込む')),
        }),
      );
    }
  }

  load();

  const cleanup = () => {
    disposed = true;
    if (chart) chart.destroy();
  };
  cleanup.update = (next) => {
    const m = isMonth(next.query.month) ? next.query.month : thisMonth;
    if (m !== month) {
      month = m;
      load();
    }
    return true;
  };
  return cleanup;
}
