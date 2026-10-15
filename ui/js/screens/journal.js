// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-05 仕訳帳 `#/journal`、詳細 `#/journal/<id>`。表とページ送り（50件）、行はEnterで詳細（右の引き出し）。
// 条件と開いている仕訳はアドレスに持つので、戻る操作で前の状態に戻る。キー: j k e v [ ]。

import { h, replace, srOnly } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen, formatDate } from '../format.js';
import { createTable } from '../components/table.js';
import { emptyState } from '../components/empty.js';
import { banner } from '../components/banner.js';
import { button, kbd } from '../components/fields.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { money } from '../components/money.js';
import { createFilters, readFilters, toQuery, toApiParams, shiftPeriod, PAGE_SIZE } from './journal-filters.js';
import { openEntryDrawer, openVoidDialog, stateBadge } from './journal-detail.js';
import { isReturnTarget, sameAddress, fromBackParam, correctHash, focusEntryRowSoon } from './journal-return.js';

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
  // 1つ前の履歴のアドレス（router.js）と、訂正を挟んだときの戻り先（アドレスの back）。板を閉じたときの行き先に使う。
  let prevHash = ctx.prevHash || null;
  let backTo = fromBackParam(ctx.query.back);
  // 一覧を読み直している間に頼まれたフォーカスは、読み終わってから移す（行が作り直されるため）。
  let loading = false;
  let pendingFocus = null;
  const cleanups = [];

  const listHash = () => ctx.router.buildHash('/journal', toQuery(filters));
  const apply = (next) => ctx.navigate(ctx.router.buildHash('/journal', toQuery(next)));

  /**
   * 板を閉じたときの行き先。viaBack が真なら history.back() で戻る（履歴の行を足さない）。
   * 一覧から開いた → 一覧へ。訂正を挟んだ → 訂正の前の画面（back）へ。ほかの画面から開いた → その画面へ。
   * どれでもない（アドレスを直に開いた、登録の後）→ いまの条件の一覧へ。
   */
  function closeTarget() {
    // 「戻る」で一覧へ戻れるのは、1つ前の履歴が一覧のときだけ（一覧から開いた詳細Aのあと、通知のリンクなどで別の詳細Bへ移っていたら、
    // 戻るとAが開き直る）。1つ前が分からない（読み込み直したあと）ときは、これまでどおり戻る。
    if (openedFromList) return { hash: listHash(), viaBack: !prevHash || sameAddress(prevHash, listHash()) };
    if (backTo) return { hash: backTo, viaBack: false };
    if (prevHash && (isReturnTarget(prevHash) || sameAddress(prevHash, listHash()))) return { hash: prevHash, viaBack: true };
    return { hash: listHash(), viaBack: false };
  }

  /**
   * 一覧の、その仕訳の行へフォーカスを移す。行が無ければ（取消済みを隠す条件で消えた、別のページ）、
   * 同じ位置（無ければ、いまの印）の行へ。利用者がフォーカスを一覧の外へ動かしていたら、何もしない。
   */
  function focusEntry(id, fallbackIndex = -1, { force = false } = {}) {
    if (loading) {
      pendingFocus = { id, fallbackIndex, force };
      return;
    }
    pendingFocus = null;
    if (document.querySelector('dialog[open]') || !items.length) return;
    const active = document.activeElement;
    if (!force && active && active !== document.body && !listHost.contains(active)) return;
    const i = items.findIndex((x) => x.id === id);
    table.focusRow(i >= 0 ? i : fallbackIndex >= 0 ? Math.min(fallbackIndex, items.length - 1) : table.currentIndex);
  }

  root.append(
    h(
      'div',
      { class: 'page-head' },
      h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '仕訳帳')),
      // 見出しの「取引を登録」は枠だけのボタン（塗りの主ボタンは、各画面の主な操作に1つだけ。ホームと同じ）。
      h('div', { class: 'page-head-actions' }, h('a', { class: 'btn hide-mobile', href: '#/entry', 'aria-keyshortcuts': 'n' }, icon('plus-square', { size: 18 }), '取引を登録', kbd('n'))),
    ),
  );
  const filterHost = h('div');
  const summary = h('p', { class: 'list-summary', 'aria-live': 'polite' });
  const listHost = h('div', { class: 'list-host' });
  // ページ送りは、一覧の上と下の両方に置く（下だけだと、50件の一覧の末尾まで送らないと次へ進めなかった）。
  const pagerTop = h('nav', { class: 'pager pager--top', 'aria-label': 'ページ送り（一覧の上）' });
  const pager = h('nav', { class: 'pager', 'aria-label': 'ページ送り（一覧の下）' });
  root.append(filterHost, summary, pagerTop, listHost, pager);

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
        // 取引先は、摘要の下の行に置く（摘要の後ろに続けると、行ごとに始まりの位置が違い、探しにくかった）。
        // 見た目は行で分けている摘要と取引先を、読み上げでも続けて読まれないように、見えない区切りを入れる。
        render: (e) => [h('span', null, e.description || '（摘要なし）'), e.partner ? [srOnly('、取引先 '), h('span', { class: 'journal-partner' }, e.partner)] : null],
      },
      // 口座の名前（サンプル銀行 普通預金）が、語の途中で折れないように（「預金」だけが次の行に残った）。
      { key: 'debit', label: '借方', className: 'c-debit', render: (e) => h('span', { class: 'keep' }, sideSummary(e.lines, 'debit')) },
      { key: 'credit', label: '貸方', className: 'c-credit', render: (e) => h('span', { class: 'keep' }, sideSummary(e.lines, 'credit')) },
      { key: 'total', label: '金額（円）', num: true, className: 'c-total', render: (e) => h('span', { class: 'amount' }, formatYen(e.total)) },
      // 札は、取消済みと訂正のある行だけ（全行の「有効」は情報が無く、目が止まらなかった）。見出しは、札と右の「›」（詳細を開く）を
      // まとめた「詳細」（「状態」だと、ほとんどの行が空に見えた）。
      { key: 'state', label: '詳細', className: 'c-state', headClass: 'c-state', render: (e) => stateBadge(e, { quiet: true }) },
      // 行を開けることを示す印（Enterかクリックで、右から詳細が開く）。見出しは読み上げだけに名前を出す（名前の無い列にしない）。
      { key: 'go', label: srOnly('詳細を開く'), stackLabel: '', className: 'c-go', render: () => icon('chevron-right', { size: 16 }) },
    ],
  });
  table.table.querySelector('th:nth-child(2)').setAttribute('aria-sort', 'descending');

  function paintPager() {
    const pages = Math.ceil(total / PAGE_SIZE);
    const go = (p) => apply({ ...filters, page: p });
    for (const nav of [pagerTop, pager]) {
      nav.replaceChildren();
      if (total <= PAGE_SIZE) continue;
      const prev = button({ label: `前の${PAGE_SIZE}件`, iconName: 'chevron-left', small: true, onClick: () => go(filters.page - 1) });
      const next = button({ label: `次の${PAGE_SIZE}件`, small: true, onClick: () => go(filters.page + 1) });
      next.append(icon('chevron-right', { size: 16 }));
      prev.disabled = filters.page <= 1;
      next.disabled = filters.page >= pages;
      nav.append(prev, h('span', { class: 'pager-text' }, `${filters.page} / ${pages} ページ`), next);
    }
  }

  async function load() {
    const my = ++seq;
    loading = true;
    const stop = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '仕訳を読み込んでいます' }));
    try {
      const res = await ctx.api.get('/entries', toApiParams(filters));
      stop();
      if (disposed || my !== seq) return;
      loading = false;
      items = res.items;
      total = res.total;
      const from = total ? (filters.page - 1) * PAGE_SIZE + 1 : 0;
      const to = Math.min(filters.page * PAGE_SIZE, total);
      // 合計は、ページの外も含む、条件に合う全件の合計（api-entries.md）。何の合計かを文で書く。
      const scope = filters.state === 'active' ? '' : filters.state === 'voided' ? '（取消済みだけ）' : '（取消済みも含む）';
      replace(summary,
        h('span', null, total ? `${formatYen(total)}件のうち ${formatYen(from)}〜${formatYen(to)}件目（日付の新しい順）` : '0件'),
        total ? h('span', { class: 'list-sum' }, `条件に合う${formatYen(total)}件の合計${scope} `, money(res.sum_amount)) : null,
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
      if (pendingFocus) focusEntry(pendingFocus.id, pendingFocus.fallbackIndex, { force: pendingFocus.force });
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      loading = false;
      pendingFocus = null;
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
      // 訂正の画面には、板を閉じたときの戻り先を持たせる（保存した後の詳細を閉じたとき、検索の条件や元帳へ戻すため）。
      editHash: () => correctHash(id, closeTarget().hash),
      onClose: (how) => {
        if (!drawer || drawer.id !== id) return;
        drawer = null;
        // 「訂正する」で閉じたときは、もう訂正の画面へ移っている。ここで戻ると、その移動を打ち消してしまう。
        if (how === 'edit') return;
        const target = closeTarget();
        openedFromList = false;
        // 戻る操作で戻せるなら、そうする（履歴に余計な行を足さない）。
        if (target.viaBack) history.back();
        else ctx.navigate(target.hash, { replace: true });
        // 一覧へ戻るなら、閉じた仕訳の行へ。ほかの画面（元帳など）なら、その画面の行が出るのを待って移す。
        if (sameAddress(target.hash, listHash())) focusEntry(id);
        else focusEntryRowSoon(id, target.hash);
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
        // 訂正を保存した後に詳細を閉じたら、いまの条件の一覧へ戻す。
        ctx.navigate(correctHash(e.id, listHash()));
        return true;
      },
      v: () => {
        const e = focused();
        if (!e || e.state === 'voided' || e.kind === 'opening' || ctx.state.isReadOnly()) return false;
        const at = table.focusedIndex();
        // 取り消すと一覧を読み直す（行が作り直されて、フォーカスが body に落ちた）。読み直した後、同じ仕訳の行へ戻す。
        openVoidDialog({ ctx, entry: e }).then((res) => {
          if (!res) return;
          pendingFocus = { id: e.id, fallbackIndex: at };
          load();
        });
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
      filterUi = createFilters({
        filters,
        accounts,
        apply,
        // 検索欄の ↓: 一覧の先頭の行へ。探し始めたなら、読み直した後に移す（いまの行は、すぐ作り直されるため）。
        onDown: (searching) => {
          if (searching) pendingFocus = { id: null, fallbackIndex: 0, force: true };
          else if (items.length && !loading) table.focusRow(0);
        },
      });
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
    prevHash = next.prevHash || null;
    backTo = fromBackParam(next.query.back);
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
