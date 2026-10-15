// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-03 未確認 `#/inbox`。製品の中心。表1枚で、Enterで行を確定する行にして次の行へ、Ctrl/Cmd+Enterで確定する。
// 絞り込み（状態・口座・文字・ページ）はアドレスに持つ。確定する行の印は画面の中だけの集合で、絞り込みやページを変えても保つ。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { createCandidates } from '../candidates.js';
import { button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { createDraftTable } from './inbox-table.js';
import { createInboxBar } from './inbox-bar.js';
import { createConfirmFlow } from './inbox-confirm.js';
import { createInboxTools } from './inbox-tools.js';
import { createIgnoreFlow } from './inbox-ignore.js';
import { ignoredTable, inboxEmpty } from './inbox-parts.js';

export const PAGE_SIZE = 200;

// 確定する行が無いときの、下の帯の案内。指で操作する幅（640px未満）には、Enter の案内は通じない。
const EMPTY_NOTE = '確定する行は、まだありません。行で Enter を押すと、確定する行になります。';
const EMPTY_NOTE_NARROW = '左の印を押すと、確定する行になります。';

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
  let draftTable = null;
  let total = 0;
  let counts = { draft: 0, ignored: 0, confirmed: 0 };
  let seq = 0;
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
  const tools = createInboxTools({ filters: () => filters, apply });
  const summary = h('p', { class: 'list-summary', 'aria-live': 'polite' });
  const listHost = h('div', { class: 'inbox-host' });
  const pager = h('nav', { class: 'pager', 'aria-label': 'ページ送り' });
  const bar = createInboxBar({ onConfirm: () => flow.run() });
  tools.panel.classList.add('inbox-panel');
  root.append(tools.el, tools.panel);
  tools.panel.append(summary, listHost, pager, bar.el);

  const rowOf = (el) => rows.find((r) => r.tr === el.closest('tr.inbox-row'));

  // 確定の流れ（保存待ち・確認・2,000件ずつ送る・結果を合わせる）は inbox-confirm.js。送る間は、表・絞り込み・ページ送りを止める。
  const flow = createConfirmFlow({
    api,
    state,
    toast,
    candidates,
    bar,
    root,
    frozen: [tools.el, listHost, pager],
    busyEl: listHost,
    ready: () => Boolean(data) && filters.status === 'draft',
    rows: () => rows,
    rowOf,
    load: () => load(),
    paintBar: () => paintBar(),
    disposed: () => disposed,
  });

  // 無視・戻す・⌘Z で戻す（inbox-ignore.js）。
  const ignoring = createIgnoreFlow({
    api,
    state,
    toast,
    rows: () => rows,
    // 番号で探す: 送っている間に一覧を読み直していたら、読み直した一覧の行を外す（無ければ、数はサーバーの値のまま）。
    drop: (id) => {
      const i = rows.findIndex((r) => r.id === id);
      if (i < 0) return;
      const [row] = rows.splice(i, 1);
      candidates.remove(id);
      row.destroy();
      row.tr.remove();
      row.msgTr.remove();
      counts = { ...counts, draft: counts.draft - 1, ignored: counts.ignored + 1 };
      total -= 1;
      paintCounts();
      paintBar();
    },
    reload: (after) => load({ after }),
    stale,
    status: () => filters.status,
    ignoredIds: () => (data && filters.status === 'ignored' ? data.items.map((d) => d.id) : []),
    ignoredButtons: () => [...listHost.querySelectorAll('.ignored-table .restore-btn')],
    focusDraftTab: () => tools.focusDraftTab(),
    focusTop: () => root.querySelector('h1').focus(),
  });

  function paintBar() {
    if (draftTable) draftTable.paint();
    const ro = state.isReadOnly();
    const sending = flow.sending;
    const empty = !ro && !sending && !candidates.size;
    const reason = ro ? state.readOnlyReason() : sending ? '確定しています…' : empty ? EMPTY_NOTE : '';
    bar.update(sending || candidates.summary(), reason, empty ? EMPTY_NOTE_NARROW : '');
  }

  function paintCounts() {
    countText.textContent = `${counts.draft}件`;
    tools.paintCounts(counts);
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

  function stale(err) {
    toast.info('他の場所で更新されました。読み込み直します。', { detail: err && err.message });
    load();
  }

  function render() {
    rows.forEach((r) => r.destroy());
    rows = [];
    draftTable = null;
    // 「無視した行」では確定しないので、確定のバーを出さない。
    bar.el.hidden = filters.status === 'ignored';
    if (!data.items.length) {
      const filtered = Boolean(filters.wallet || filters.q);
      listHost.replaceChildren(inboxEmpty({ status: filters.status, filtered, onClear: () => apply({ wallet: '', q: '' }) }));
      return;
    }
    if (filters.status === 'ignored') {
      listHost.replaceChildren(ignoredTable({ items: data.items, thisYear, readOnly: state.isReadOnly(), onRestore: (d) => ignoring.restore(d.id) }));
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
      onIgnore: (row) => ignoring.ignore(row),
      onStale: stale,
      onSaveState: (k) => bar.setSaveState(k),
      onLast: () => bar.confirmButton.focus(),
    });
    rows = t.rows;
    draftTable = t;
    listHost.replaceChildren(t.el);
    t.layout(listHost.getBoundingClientRect().width);
  }

  // 入れ物の幅が、表とカードの境目をまたいだら、行の欄の並び（Tabの順）を合わせ直す。
  const resize = new ResizeObserver((entries) => {
    if (draftTable && entries.length) draftTable.layout(entries[entries.length - 1].contentRect.width);
  });
  resize.observe(listHost);

  /** いまフォーカスのある行（番号と列と位置）。読み直したあとに、同じ行へ戻すため。 */
  function focusSpot() {
    const el = document.activeElement;
    const row = el && el.closest ? rowOf(el) : null;
    const td = row && el.closest('td');
    return row ? { id: row.id, index: rows.indexOf(row), col: td ? td.dataset.col : null } : null;
  }

  /**
   * 一覧を読み直す。after を渡せば、描いた後に呼ぶ（戻した行へのフォーカスなど）。渡さなければ、フォーカスのあった行へ戻す
   * （その行が無くなっていれば、同じ位置の行）。
   */
  async function load({ after } = {}) {
    const my = ++seq;
    const stop = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '未確認の明細を読み込んでいます' }));
    const spot = focusSpot();
    try {
      const res = await api.get('/drafts', { status: filters.status, wallet: filters.wallet, q: filters.q, limit: PAGE_SIZE, offset: (filters.page - 1) * PAGE_SIZE });
      stop();
      if (disposed || my !== seq) return;
      data = { ...data, items: res.items };
      total = res.total;
      counts = res.counts || counts;
      // 絞り込みが無く1ページに収まる＝未確認の全部の行が見えている。そこに無い印（ほかで確定・無視された行）は外す。
      if (filters.status === 'draft' && !filters.wallet && !filters.q && filters.page === 1 && total <= PAGE_SIZE) {
        candidates.keepOnly(new Set(res.items.map((d) => d.id)));
      }
      render();
      paintCounts();
      paintPager();
      paintBar();
      if (after) after();
      else if (spot && rows.length) (rows.find((r) => r.id === spot.id) || rows[Math.min(spot.index, rows.length - 1)]).focus(spot.col || 'account');
    } catch (err) {
      stop();
      if (disposed || my !== seq) return;
      listHost.replaceChildren(banner({ kind: 'danger', title: '未確認の明細を読み込めませんでした', body: err.message, actions: h('div', null, button({ label: 'もう一度読み込む', small: true, onClick: () => load() })) }));
    }
  }

  const onDocKey = (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !e.altKey && !e.shiftKey && !document.querySelector('dialog[open]')) {
      e.preventDefault();
      flow.run();
      return;
    }
    ignoring.onKey(e);
  };
  document.addEventListener('keydown', onDocKey);
  const unsub = state.subscribe('status', () => paintBar());

  const stopSkel = delayedSkeleton(listHost, () => skeletonBlock({ lines: 6, label: '未確認の明細を読み込んでいます' }));
  const fresh = { force: true };
  Promise.all([state.load('settings', fresh), state.load('accounts', fresh), state.load('partners', fresh)])
    .then(([settings, accounts, partners]) => {
      stopSkel();
      if (disposed) return;
      data = { settings, accounts, partners, items: [] };
      tools.setWallets(accounts);
      tools.sync(filters);
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
    resize.disconnect();
    unsub();
    bar.destroy();
    rows.forEach((r) => r.destroy());
  };
  cleanup.update = (nextCtx) => {
    const nextFilters = readFilters(nextCtx.query);
    if (JSON.stringify(nextFilters) !== JSON.stringify(filters)) {
      filters = nextFilters;
      if (data) {
        tools.sync(filters);
        load();
      }
    }
    return true;
  };
  return cleanup;
}
