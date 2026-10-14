// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-03 未確認 `#/inbox`。製品の中心。表1枚で、Enterで行を確定候補にして次の行へ、Ctrl/Cmd+Enterで確定する。
// 絞り込み（状態・口座・文字・ページ）はアドレスに持つ。確定候補の印は画面の中だけの集合で、絞り込みやページを変えても保つ。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { createCandidates } from '../candidates.js';
import { createTabs } from '../components/tabs.js';
import { selectInput, textInput, button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createDraftTable } from './inbox-table.js';
import { createInboxBar, confirmCandidates } from './inbox-bar.js';
import { ignoredTable, inboxEmpty } from './inbox-parts.js';
import { walletItems } from './entry-options.js';

export const PAGE_SIZE = 200;

function readFilters(q) {
  const page = Number(q.page);
  return { status: q.status === 'ignored' ? 'ignored' : 'draft', wallet: q.wallet || '', q: q.q || '', page: Number.isInteger(page) && page > 1 ? page : 1 };
}

function toQuery(f) {
  return { status: f.status === 'ignored' ? 'ignored' : null, wallet: f.wallet || null, q: f.q || null, page: f.page > 1 ? f.page : null };
}

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let filters = readFilters(ctx.query);
  const candidates = createCandidates();
  let data = null;
  let rows = [];
  let total = 0;
  let counts = { draft: 0, ignored: 0, confirmed: 0 };
  let seq = 0;
  let busy = false;
  let disposed = false;
  const thisYear = String(new Date().getFullYear());
  const apply = (patch) => ctx.navigate(ctx.router.buildHash('/inbox', toQuery({ ...filters, page: 1, ...patch })));

  const countText = h('span', { class: 'page-count', 'aria-live': 'polite' });
  root.append(
    h(
      'div',
      { class: 'page-head' },
      h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '未確認'), countText),
      h('div', { class: 'page-head-actions' }, h('a', { class: 'btn', href: '#/import' }, icon('import', { size: 18 }), '明細を取り込む')),
    ),
  );
  const tabs = createTabs({ label: '表示する明細', tabs: [{ id: 'draft', label: '未確認' }, { id: 'ignored', label: '無視した行' }], selected: filters.status, onChange: (id) => apply({ status: id }) });
  const walletSel = selectInput({ 'aria-label': '口座で絞り込む', class: 'inbox-wallet' }, [{ value: '', label: 'すべての口座' }]);
  walletSel.addEventListener('change', () => apply({ wallet: walletSel.value }));
  const search = textInput({ type: 'search', placeholder: '摘要・取引先・金額で探す', 'aria-label': '明細を文字で探す', dataset: { search: '1' } });
  const commitSearch = () => search.value.trim() !== filters.q && apply({ q: search.value.trim() });
  search.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.key !== 'Enter') return;
    e.preventDefault();
    commitSearch();
  });
  search.addEventListener('change', commitSearch);
  const tools = h('div', { class: 'card inbox-tools', role: 'search', 'aria-label': '未確認の絞り込み' }, tabs.el, h('div', { class: 'inbox-filters' }, walletSel, h('div', { class: 'search-box' }, icon('search', { size: 18 }), search, h('kbd', { 'aria-hidden': 'true' }, '/'))));
  const summary = h('p', { class: 'list-summary', 'aria-live': 'polite' });
  const listHost = h('div', { class: 'inbox-host' });
  const pager = h('nav', { class: 'pager', 'aria-label': 'ページ送り' });
  const bar = createInboxBar({ onConfirm: () => confirmAll() });
  tabs.panel.classList.add('inbox-panel');
  root.append(tools, tabs.panel);
  tabs.panel.append(summary, listHost, pager, bar.el);

  const rowOf = (el) => rows.find((r) => r.tr === el.closest('tr.inbox-row'));

  function paintBar() {
    const ro = state.isReadOnly();
    const reason = ro ? state.readOnlyReason() : busy ? '確定しています…' : candidates.size ? '' : '確定候補はまだありません。行で Enter を押すと、確定候補になります。';
    bar.update(candidates.summary(), reason);
  }

  function paintCounts() {
    countText.textContent = `${counts.draft}件`;
    tabs.el.querySelector('[data-tab="draft"]').textContent = `未確認 ${counts.draft}件`;
    tabs.el.querySelector('[data-tab="ignored"]').textContent = `無視した行 ${counts.ignored}件`;
    const from = total ? (filters.page - 1) * PAGE_SIZE + 1 : 0;
    summary.textContent = total ? `${total}件のうち ${from}〜${Math.min(filters.page * PAGE_SIZE, total)}件目（日付の古い順）` : '';
  }

  function paintPager() {
    pager.replaceChildren();
    if (total <= PAGE_SIZE) return;
    const pages = Math.ceil(total / PAGE_SIZE);
    const prev = button({ label: `前の${PAGE_SIZE}件`, iconName: 'chevron-left', small: true, onClick: () => apply({ page: filters.page - 1 }) });
    const next = button({ label: `次の${PAGE_SIZE}件`, small: true, onClick: () => apply({ page: filters.page + 1 }) });
    next.append(icon('chevron-right', { size: 16 }));
    prev.disabled = filters.page <= 1;
    next.disabled = filters.page >= pages;
    pager.append(prev, h('span', { class: 'pager-text' }, `${filters.page} / ${pages} ページ`), next);
  }

  async function ignore(row) {
    if (state.isReadOnly()) return;
    const i = rows.indexOf(row);
    try {
      await api.post(`/drafts/${row.id}/ignore`);
    } catch (err) {
      if (err.code === 'already_confirmed') return stale(err);
      row.showError(err.message);
      return undefined;
    }
    candidates.remove(row.id);
    row.destroy();
    row.tr.remove();
    row.msgTr.remove();
    rows.splice(i, 1);
    counts = { ...counts, draft: counts.draft - 1, ignored: counts.ignored + 1 };
    total -= 1;
    paintCounts();
    paintBar();
    state.load('status', { force: true }).catch(() => {});
    toast.success(`「${row.draft.description}」を取り込まない行にしました。`, { action: { label: '元に戻す', onClick: () => restore(row.id) } });
    const target = rows[Math.min(i, rows.length - 1)];
    if (target) target.focus('cand');
    else if (!rows.length) load();
    return undefined;
  }

  async function restore(id) {
    try {
      await api.post(`/drafts/${id}/restore`);
      toast.success('未確認に戻しました。');
      state.load('status', { force: true }).catch(() => {});
      load();
    } catch (err) {
      toast.error(err.message);
    }
  }

  function stale(err) {
    toast.info('他の場所で更新されました。読み込み直します。', { detail: err && err.message });
    load();
  }

  function render() {
    rows.forEach((r) => r.destroy());
    rows = [];
    // 「無視した行」では確定しないので、確定のバーを出さない。
    bar.el.hidden = filters.status === 'ignored';
    if (!data.items.length) {
      const filtered = Boolean(filters.wallet || filters.q);
      listHost.replaceChildren(inboxEmpty({ status: filters.status, filtered, onClear: () => apply({ wallet: '', q: '' }) }));
      return;
    }
    if (filters.status === 'ignored') {
      listHost.replaceChildren(ignoredTable({ items: data.items, thisYear, readOnly: state.isReadOnly(), onRestore: (d) => restore(d.id) }));
      return;
    }
    const t = createDraftTable({
      items: data.items,
      settings: data.settings,
      accounts: data.accounts,
      partners: data.partners,
      readOnly: state.isReadOnly(),
      thisYear,
      api,
      candidates,
      onCandidate: paintBar,
      onIgnore: ignore,
      onStale: stale,
      onSaveState: (k) => bar.setSaveState(k),
      onLast: () => bar.confirmButton.focus(),
    });
    rows = t.rows;
    listHost.replaceChildren(t.el);
  }

  async function load() {
    const my = ++seq;
    const stop = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '未確認の明細を読み込んでいます' }));
    const focus = document.activeElement && document.activeElement.closest ? document.activeElement.closest('tr.inbox-row') : null;
    const focusCol = focus && document.activeElement.closest('td') ? document.activeElement.closest('td').dataset.col : null;
    const focusIndex = focus ? rows.findIndex((r) => r.tr === focus) : -1;
    try {
      const res = await api.get('/drafts', { status: filters.status, wallet: filters.wallet, q: filters.q, limit: PAGE_SIZE, offset: (filters.page - 1) * PAGE_SIZE });
      stop();
      if (disposed || my !== seq) return;
      data = { ...data, items: res.items };
      total = res.total;
      counts = res.counts || counts;
      render();
      paintCounts();
      paintPager();
      paintBar();
      if (focusIndex >= 0 && rows.length) rows[Math.min(focusIndex, rows.length - 1)].focus(focusCol || 'account');
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      listHost.replaceChildren(banner({ kind: 'danger', title: '未確認の明細を読み込めませんでした', body: err.message, actions: h('div', null, button({ label: 'もう一度読み込む', small: true, onClick: () => load() })) }));
    }
  }

  async function confirmAll() {
    if (busy || !data || filters.status !== 'draft') return;
    if (state.isReadOnly()) return paintBar();
    const active = document.activeElement;
    const row = active ? rowOf(active) : null;
    if (row) row.commit(active.closest('td') ? active.closest('td').dataset.col : null);
    if (!candidates.size) {
      bar.update(candidates.summary(), '確定候補がありません。行で Enter を押して、確定候補にしてください。');
      return undefined;
    }
    await Promise.all(rows.map((r) => r.flush()));
    if (!(await confirmCandidates(candidates.summary()))) return undefined;
    busy = true;
    paintBar();
    try {
      const res = await api.post('/drafts/confirm', { ids: candidates.ids() });
      res.confirmed.forEach((c) => candidates.remove(c.id));
      if (res.confirmed.length) {
        const nos = res.confirmed.map((c) => c.no).sort((a, b) => a - b);
        toast.success(`${res.confirmed.length}件を確定しました（No.${nos[0]}〜No.${nos[nos.length - 1]}）。`, { action: { label: '仕訳帳を見る', href: '#/journal' } });
      }
      if (res.failed.length) toast.error(`${res.failed.length}件は確定できませんでした。行の下の理由を確かめて、直してからもう一度確定してください。`);
      state.load('status', { force: true }).catch(() => {});
      busy = false;
      await load();
      res.failed.forEach((f) => {
        const r = rows.find((x) => x.id === f.id);
        if (r) r.showError(f.error.message);
      });
    } catch (err) {
      busy = false;
      if (err.isStale || err.code === 'already_confirmed') stale(err);
      else toast.error(err.message);
    } finally {
      busy = false;
      paintBar();
    }
    return undefined;
  }

  const onDocKey = (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && !document.querySelector('dialog[open]')) {
      e.preventDefault();
      confirmAll();
    }
  };
  document.addEventListener('keydown', onDocKey);
  const unsub = state.subscribe('status', () => paintBar());

  function syncControls() {
    tabs.select(filters.status);
    walletSel.value = filters.wallet;
    if (document.activeElement !== search) search.value = filters.q;
  }

  const stopSkel = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '未確認の明細を読み込んでいます' }));
  const fresh = { force: true };
  Promise.all([state.load('settings', fresh), state.load('accounts', fresh), state.load('partners', fresh)])
    .then(([settings, accounts, partners]) => {
      stopSkel();
      if (disposed) return;
      data = { settings, accounts, partners, items: [] };
      walletSel.replaceChildren(h('option', { value: '' }, 'すべての口座'), ...walletItems(accounts).map((w) => h('option', { value: w.value }, w.label)));
      syncControls();
      load();
    })
    .catch((err) => {
      stopSkel();
      if (!disposed) listHost.replaceChildren(banner({ kind: 'danger', title: '未確認の画面を準備できませんでした', body: err.message }));
    });
  paintBar();

  const cleanup = () => {
    disposed = true;
    document.removeEventListener('keydown', onDocKey);
    unsub();
    rows.forEach((r) => r.destroy());
  };
  cleanup.update = (nextCtx) => {
    const nextFilters = readFilters(nextCtx.query);
    if (JSON.stringify(nextFilters) !== JSON.stringify(filters)) {
      filters = nextFilters;
      if (data) {
        syncControls();
        load();
      }
    }
    return true;
  };
  return cleanup;
}

