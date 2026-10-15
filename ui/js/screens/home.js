// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-01 ホーム `#/`。一番上は「次にやること」（AC-07.1）。数字は1回の通信（/reports/dashboard）で受け取る。
// 月はアドレスの ?month= に持つ（戻る操作で前の月に戻れる）。今月のときは書かない。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatMonth } from '../format.js';
import { todayIso, shiftMonth } from '../input-parse.js';
import { createChart } from '../components/chart.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { banner } from '../components/banner.js';
import { iconButton, kbd, shortcutKeys } from '../components/fields.js';
import { nextActions, kpiCard, walletsCard, topExpensesCard, ledgerCard, taxCard } from './home-parts.js';

const isMonth = (s) => /^\d{4}-(0[1-9]|1[0-2])$/.test(s || '');
/** 開業日の月（「2026-01」）。設定がまだ読めていなければ null。 */
const openedMonth = (s) => (s && isMonth(String(s.opened_on || '').slice(0, 7)) ? s.opened_on.slice(0, 7) : null);

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
  // 選べる月は、開業日の月から今月まで（開業前の月は、何も記録できないので並べない）と、今月より後で記録のある月。
  let firstMonth = openedMonth(ctx.state.peek('settings'));
  let laterMonths = [];

  const select = h('select', { class: 'select month-select', 'aria-label': '表示する月' });
  // 前後の月の矢印は、枠を付けてボタンに見せる（枠が無いと、小さな文字に見えた）。並びの中の隣の月へ移る。
  const prevBtn = iconButton({ iconName: 'chevron-left', label: '前の月', className: 'month-step', onClick: () => go(neighbor(-1)) });
  const nextBtn = iconButton({ iconName: 'chevron-right', label: '次の月', className: 'month-step', onClick: () => go(neighbor(1)) });
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
        // 塗りの主ボタンは「次にやること」の中の1つだけにする（ここも塗ると、焦点が2つに割れた）。
        h('a', { class: 'btn hide-mobile', href: '#/entry', 'aria-keyshortcuts': shortcutKeys('n') }, icon('plus-square', { size: 18 }), '取引を登録', kbd('n')),
      ),
    ),
    body,
  );

  /** 選べる月（新しい順）。アドレスで並びの外の月を開いたときは、その月も入れる。 */
  function months() {
    const start = firstMonth && firstMonth <= thisMonth ? firstMonth : thisMonth;
    const out = [...laterMonths].sort().reverse();
    for (let m = thisMonth; m >= start; m = shiftMonth(m, -1)) out.push(m);
    if (!out.includes(month)) out.push(month);
    return out.sort().reverse();
  }

  /** 並びの中で、いまの月の隣（-1 は前、1 は後）。無ければ null。 */
  function neighbor(step) {
    const list = months().reverse();
    const i = list.indexOf(month);
    return list[i + step] || null;
  }

  function go(m) {
    if (!m || (m > thisMonth && !laterMonths.includes(m))) return;
    ctx.navigate(ctx.router.buildHash('/', { month: m === thisMonth ? null : m }));
  }

  /**
   * 前の月の長さの書き方。終わった月は「1か月分」、今月は「途中」。開業した月で、開業日が1日でなければ書かない
   * （丸1か月ではないが、開業日より前の日付の記録もありうるので、日付の範囲も書かない）。
   */
  function spanOf(prevMonth) {
    if (prevMonth === thisMonth) return '途中';
    if (prevMonth > thisMonth) return '';
    const opened = String((ctx.state.peek('settings') || {}).opened_on || '');
    return opened.slice(0, 7) === prevMonth && opened.slice(8, 10) !== '01' ? '' : '1か月分';
  }

  /**
   * 年の利益の期間の始め。開業日がその年の途中（4月1日など）で、開業の月より前に記録が無ければ、開業の月から書く
   * （「1月〜10月」だと、開業前の月も集計に入っているように見えた）。記録があれば、集計どおり「1月」。
   */
  function startLabel(dash) {
    const opened = String((ctx.state.peek('settings') || {}).opened_on || '');
    const om = opened.slice(0, 7);
    if (!isMonth(om) || om.slice(0, 4) !== String(dash.fiscal_year) || om.endsWith('-01') || om > month) return '1月';
    const before = (dash.series || []).filter((m) => m.month < om);
    return before.every((m) => !m.revenue && !m.expense) ? `${Number(om.slice(5, 7))}月` : '1月';
  }

  function fillMonths() {
    select.replaceChildren(...months().map((m) => h('option', { value: m }, formatMonth(m))));
    select.value = month;
    prevBtn.disabled = !neighbor(-1);
    nextBtn.disabled = !neighbor(1);
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
    // 今月（と、記録のある先の月）は途中の月。前の月と比べた差は出さず、前の月の実額を添える（過去の月は今までどおり差を出す）。
    // 前の月が開業前なら、比べるものが無いので何も出さない。
    const prevMonth = shiftMonth(month, -1);
    const partial = month >= thisMonth ? { month: `${Number(prevMonth.slice(5, 7))}月`, span: spanOf(prevMonth) } : null;
    const beforeOpening = Boolean(partial && firstMonth && prevMonth < firstMonth);
    const prevOf = (v) => (beforeOpening ? undefined : v);
    parts.push(
      h(
        'section',
        { class: 'kpis', 'aria-label': `${formatMonth(month)}の数字` },
        kpiCard({ label: `${mLabel}の売上`, value: ms.revenue, prev: prevOf(ms.prev.revenue), partial }),
        kpiCard({ label: `${mLabel}の経費`, value: ms.expense, prev: prevOf(ms.prev.expense), partial }),
        kpiCard({ label: `${mLabel}の利益`, value: ms.profit, prev: prevOf(ms.prev.profit), profit: true, partial }),
        kpiCard({
          label: `${dash.fiscal_year}年の利益`,
          value: dash.ytd.profit,
          profit: true,
          period: `${startLabel(dash)}〜${mLabel}`,
          notes: [
            ['売上', dash.ytd.revenue],
            ['経費', dash.ytd.expense],
          ],
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
        walletsCard(dash.wallets, dash.next_actions.inbox_draft),
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
      const [dash, status, settings] = await Promise.all([ctx.api.get('/reports/dashboard', { month }), ctx.state.load('status', { force: true }), ctx.state.load('settings')]);
      stop();
      if (disposed || my !== seq) return;
      firstMonth = openedMonth(settings);
      // 今月より後で記録のある月は、今年の推移（今年の月を見ているときの応え）から拾う。
      if (dash.fiscal_year === Number(thisMonth.slice(0, 4))) laterMonths = dash.series.filter((m) => m.month > thisMonth && (m.revenue || m.expense)).map((m) => m.month);
      fillMonths();
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
