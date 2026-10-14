// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-05 仕訳帳 `#/journal`、詳細 `#/journal/<id>`。表とページ送り（50件）、行はEnterで詳細（右の引き出し）。
// 条件と開いている仕訳はアドレスに持つので、戻る操作で前の状態に戻る。キー: j k e v [ ]。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen, formatDate } from '../format.js';
import { createTable } from '../components/table.js';
import { emptyState } from '../components/empty.js';
import { banner } from '../components/banner.js';
import { button } from '../components/fields.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createFilters, readFilters, toQuery, toApiParams, shiftPeriod, PAGE_SIZE } from './journal-filters.js';
import { openEntryDrawer, openVoidDialog, stateBadge } from './journal-detail.js';

/** 借方か貸方の科目。複合仕訳は「ほか2行」と出す。 */
export function sideSummary(lines, side) {
  const xs = lines.filter((l) => l.side === side);
  if (!xs.length) return '';
  return xs.length > 1 ? `${xs[0].account_name} ほか${xs.length - 1}行` : xs[0].account_name;
}

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  let filters = readFilters(ctx.query);
  let items = [];
  let total = 0;
  let seq = 0;
  let disposed = false;
  let drawer = null;
  let openedFromList = false;
  const cleanups = [];

  const listHash = () => ctx.router.buildHash('/journal', toQuery(filters));
  const apply = (next) => ctx.navigate(ctx.router.buildHash('/journal', toQuery(next)));

  root.append(
    h(
      'div',
      { class: 'page-head' },
      h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '仕訳帳')),
      h('div', { class: 'page-head-actions' }, h('a', { class: 'btn btn--primary hide-mobile', href: '#/entry' }, icon('plus-square', { size: 18 }), '取引を登録', h('kbd', null, 'n'))),
    ),
  );
  const filterHost = h('div');
  const summary = h('p', { class: 'list-summary', 'aria-live': 'polite' });
  const listHost = h('div', { class: 'list-host' });
  const pager = h('nav', { class: 'pager', 'aria-label': 'ページ送り' });
  root.append(filterHost, summary, listHost, pager);

  const table = createTable({
    caption: '仕訳の一覧（Enterで詳細を開く）',
    className: 'journal-table',
    // 狭い幅の並べ替えは、画面の幅ではなく入れ物の幅で決める（screens-journal.css のコンテナクエリ）。
    stack: false,
    rowKey: (e) => e.id,
    rowClass: (e) => (e.state === 'voided' ? 'is-voided' : null),
    onOpen: (e) => openDetail(e.id),
    columns: [
      { key: 'no', label: 'No', num: true, className: 'c-no', render: (e) => String(e.no) },
      { key: 'date', label: '日付', className: 'c-date', render: (e) => formatDate(e.date) },
      {
        key: 'description',
        label: '摘要',
        className: 'wrap c-desc',
        stackLabel: '',
        render: (e) => [h('span', null, e.description || '（摘要なし）'), e.partner ? h('span', { class: 'sub-inline' }, e.partner) : null],
      },
      { key: 'debit', label: '借方', className: 'c-debit', render: (e) => sideSummary(e.lines, 'debit') },
      { key: 'credit', label: '貸方', className: 'c-credit', render: (e) => sideSummary(e.lines, 'credit') },
      { key: 'total', label: '金額', num: true, className: 'c-total', render: (e) => h('span', { class: 'amount' }, formatYen(e.total)) },
      { key: 'state', label: '状態', className: 'c-state', render: (e) => stateBadge(e) },
    ],
  });
  table.table.querySelector('th:nth-child(2)').setAttribute('aria-sort', 'descending');

  function paintPager() {
    pager.replaceChildren();
    if (total <= PAGE_SIZE) return;
    const pages = Math.ceil(total / PAGE_SIZE);
    const go = (p) => apply({ ...filters, page: p });
    const prev = button({ label: `前の${PAGE_SIZE}件`, iconName: 'chevron-left', small: true, onClick: () => go(filters.page - 1) });
    const next = button({ label: `次の${PAGE_SIZE}件`, small: true, onClick: () => go(filters.page + 1) });
    next.append(icon('chevron-right', { size: 16 }));
    prev.disabled = filters.page <= 1;
    next.disabled = filters.page >= pages;
    pager.append(prev, h('span', { class: 'pager-text' }, `${filters.page} / ${pages} ページ`), next);
  }

  async function load() {
    const my = ++seq;
    const stop = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '仕訳を読み込んでいます' }));
    try {
      const res = await ctx.api.get('/entries', toApiParams(filters));
      stop();
      if (disposed || my !== seq) return;
      items = res.items;
      total = res.total;
      const from = total ? (filters.page - 1) * PAGE_SIZE + 1 : 0;
      const to = Math.min(filters.page * PAGE_SIZE, total);
      replace(summary, 
        h('span', null, total ? `${total}件のうち ${from}〜${to}件目` : '0件'),
        total ? h('span', null, `合計 ${formatYen(res.sum_amount)}円`) : null,
      );
      if (!items.length) {
        const filtered = Object.keys(toQuery({ ...filters, page: 1 })).length > 0;
        listHost.replaceChildren(
          h(
            'div',
            { class: 'card' },
            emptyState(
              filtered
                ? { iconName: 'search', title: '条件に合う仕訳はありません', body: '条件をゆるめるか、「条件を消す」で全部の仕訳に戻してください。', actions: [{ label: '条件を消す', href: '#/journal' }] }
                : { iconName: 'book', title: 'まだ仕訳がありません', body: '取引を登録するか、明細を取り込んで確定すると、ここに並びます。', actions: [{ label: '取引を登録', href: '#/entry', primary: true, kbd: 'n' }] },
            ),
          ),
        );
      } else {
        table.setRows(items);
        listHost.replaceChildren(table.el);
      }
      paintPager();
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      listHost.replaceChildren(banner({ kind: 'danger', title: '仕訳を読み込めませんでした', body: err.message, actions: h('div', null, button({ label: 'もう一度読み込む', small: true, onClick: load })) }));
    }
  }

  function openDetail(id) {
    openedFromList = true;
    ctx.navigate(ctx.router.buildHash(`/journal/${encodeURIComponent(id)}`, toQuery(filters)));
  }

  // アドレスの仕訳の id に合わせて、引き出しを開け閉めする。
  function syncDrawer(id) {
    if (drawer && drawer.id === id) return;
    if (drawer) {
      const d = drawer;
      drawer = null;
      d.close();
    }
    if (!id) return;
    drawer = openEntryDrawer({
      ctx,
      id,
      onChanged: () => load(),
      onClose: (how) => {
        if (!drawer || drawer.id !== id) return;
        drawer = null;
        // 「訂正する」で閉じたときは、もう訂正の画面へ移っている。ここで戻ると、その移動を打ち消してしまう。
        if (how === 'edit') return;
        // 一覧から開いたなら、戻る操作で一覧に戻す（履歴に余計な行を足さない）。
        if (openedFromList) history.back();
        else ctx.navigate(listHash(), { replace: true });
        openedFromList = false;
      },
    });
  }

  const focused = () => {
    const i = table.focusedIndex();
    return i >= 0 ? items[i] : null;
  };
  cleanups.push(
    ctx.keys.register({
      j: () => (items.length ? table.focusRow(table.focusedIndex() < 0 ? 0 : table.focusedIndex() + 1) : false),
      k: () => (items.length ? table.focusRow(table.focusedIndex() < 0 ? 0 : table.focusedIndex() - 1) : false),
      e: () => {
        const e = focused();
        if (!e || e.state === 'voided' || e.kind === 'opening' || ctx.state.isReadOnly()) return false;
        ctx.navigate(`#/entry/${e.id}`);
        return true;
      },
      v: () => {
        const e = focused();
        if (!e || e.state === 'voided' || e.kind === 'opening' || ctx.state.isReadOnly()) return false;
        openVoidDialog({ ctx, entry: e }).then((res) => res && load());
        return true;
      },
      '[': () => apply(shiftPeriod(filters, -1)),
      ']': () => apply(shiftPeriod(filters, 1)),
    }),
  );

  const stopFilters = delayedSkeleton(filterHost, () => skeletonBlock({ lines: 2 }));
  let filterUi = null;
  ctx.state
    .load('accounts')
    .then((accounts) => {
      stopFilters();
      if (disposed) return;
      filterUi = createFilters({ filters, accounts, apply });
      filterHost.replaceChildren(filterUi.el);
    })
    .catch(() => stopFilters());
  load();
  syncDrawer(ctx.params.id || null);

  const cleanup = () => {
    disposed = true;
    cleanups.forEach((fn) => fn());
    if (filterUi) filterUi.destroy();
    if (drawer) {
      const d = drawer;
      drawer = null;
      d.close();
    }
  };
  cleanup.update = (next) => {
    const nextFilters = readFilters(next.query);
    const changed = JSON.stringify(nextFilters) !== JSON.stringify(filters);
    filters = nextFilters;
    if (changed) {
      if (filterUi) filterUi.sync(filters);
      load();
    }
    syncDrawer(next.params.id || null);
    return true;
  };
  return cleanup;
}
