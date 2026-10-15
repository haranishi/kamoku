// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-08 勘定科目 `#/accounts`。分類（資産・負債・資本・収益・費用）ごとに並べ、名前と読みとコードで探せる。
// 使わない科目は無効にできる（過去の仕訳は変わらない）。科目ごとに既定の税区分と事業割合。行で Enter を押すと直す。
// 探す言葉と「使っていない科目も出す」はアドレスに持つ。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { TAX_LABELS } from '../account-defaults.js';
import { treeRows, chartCompare } from '../report-tree.js';
import { filterItems } from '../kana.js';
import { createTable } from '../components/table.js';
import { textInput, button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { emptyValue } from '../components/money.js';
import { accountDialog, KIND_LABEL } from './master-forms.js';
import { phrase } from '../components/phrase.js';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let accounts = [];
  let q = ctx.query.q || '';
  let showAll = ctx.query.all === '1';
  let disposed = false;
  const apply = (patch) => ctx.navigate(ctx.router.buildHash('/accounts', { q, all: showAll ? '1' : null, ...patch }), { replace: true });

  const search = textInput({ type: 'search', value: q, placeholder: '名前・読み・コードで探す', 'aria-label': '科目を探す', dataset: { search: '1' } });
  search.addEventListener('input', () => {
    q = search.value.trim();
    render();
  });
  search.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.key !== 'Enter') return;
    e.preventDefault();
    apply({ q: search.value.trim() });
  });
  search.addEventListener('change', () => apply({ q: search.value.trim() }));
  const allBox = h('input', { type: 'checkbox', checked: showAll });
  allBox.addEventListener('change', () => apply({ all: allBox.checked ? '1' : null }));
  const body = h('div', { class: 'masters' });
  root.append(
    h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '勘定科目'), h('p', { class: 'muted small' }, phrase('標準の科目は、国税庁の青色申告決算書の項目名に合わせてあります。'))), h('div', { class: 'page-head-actions' }, button({ label: '科目を足す', variant: 'primary', iconName: 'plus', onClick: () => add() }))),
    h('div', { class: 'masters-tools', role: 'search', 'aria-label': '科目の絞り込み' }, h('div', { class: 'search-box' }, icon('search', { size: 18 }), search, h('kbd', { 'aria-hidden': 'true' }, '/')), h('label', { class: 'check' }, allBox, h('span', null, '使っていない科目も出す'))),
    body,
  );

  async function add() {
    const saved = await accountDialog({ api, accounts });
    if (!saved) return;
    toast.success(`科目「${saved.name}」を足しました。`);
    load();
  }

  async function edit(a) {
    const saved = await accountDialog({ api, account: a, accounts });
    if (!saved) return;
    toast.success(`「${saved.name}」を保存しました。`);
    load();
  }

  function section(kind, rows) {
    const t = createTable({
      caption: `${KIND_LABEL[kind]}の科目（行で Enter を押すと直す）`,
      className: 'accounts-table',
      rowKey: (r) => r.code,
      rowClass: (r) => (r.enabled === false ? 'is-off' : null),
      onOpen: (r) => edit(accounts.find((a) => a.code === r.code)),
      columns: [
        {
          key: 'name',
          label: '名前',
          stackLabel: '',
          className: 'acct-cell',
          // 読みとコードは、狭い幅（1279px以下）では名前の下の小さな2行目に畳む（列にすると「…」で切れた）。
          render: (r) => {
            const main = h(
              'div',
              { class: 'acct-main' },
              h('span', { class: ['acct-name', r.parent && 'is-parent'] }, h('span', { class: 'keep' }, r.name), r.wallet ? h('span', { class: 'badge badge--muted' }, '口座') : null, r.enabled === false ? h('span', { class: 'badge badge--muted' }, '使っていない') : null),
              h('span', { class: 'acct-sub' }, r.reading ? h('span', { class: 'keep' }, r.reading) : null, h('code', { class: 'mono' }, r.code)),
            );
            if (r.depth) main.style.setProperty('--depth', String(r.depth));
            return main;
          },
        },
        { key: 'reading', label: '読み', className: 'acct-col-wide', headClass: 'acct-col-wide', render: (r) => (r.reading ? h('span', { class: 'keep' }, r.reading) : emptyValue()) },
        { key: 'code', label: 'コード', className: 'acct-col-wide', headClass: 'acct-col-wide', render: (r) => h('code', { class: 'mono' }, r.code) },
        { key: 'default_tax', label: '既定の税区分', className: 'acct-tax', render: (r) => (r.kind === 'revenue' || r.kind === 'expense' ? h('span', { class: 'keep' }, r.default_tax ? TAX_LABELS[r.default_tax] || r.default_tax : '対象外') : emptyValue()) },
        { key: 'default_pct', label: '既定の割合', num: true, className: 'acct-pct', render: (r) => (r.kind === 'expense' ? `${r.default_pct ?? 100}%` : emptyValue()) },
      ],
      rows,
    });
    return h('section', { class: 'masters-section', 'aria-labelledby': `kind-${kind}` }, h('h2', { id: `kind-${kind}`, class: 'card-title' }, KIND_LABEL[kind], h('span', { class: 'muted small' }, `${rows.filter((r) => !r.synthetic).length}件`)), t.el);
  }

  function render() {
    const items = accounts.filter((a) => showAll || a.enabled).map((a) => ({ ...a, value: a.code, label: a.name }));
    const hits = q ? filterItems(items, q) : items;
    if (!hits.length) {
      replace(body, h('div', { class: 'card' }, emptyState({ iconName: 'search', title: '当てはまる科目はありません', body: showAll ? '探す言葉を変えてください。' : '探す言葉を変えるか、「使っていない科目も出す」を選んでください。' })));
      return;
    }
    const all = new Map(accounts.map((a) => [a.code, a]));
    const parts = Object.keys(KIND_LABEL).map((kind) => {
      const rows = hits.filter((a) => a.kind === kind).sort(chartCompare);
      if (!rows.length) return null;
      // 親が絞り込みで外れても、場所が分かるように親の行を出す（treeRows が科目から作る）。
      const tree = treeRows(rows, accounts, []).map((r) => ({ ...(all.get(r.code) || {}), ...r, synthetic: !rows.some((x) => x.code === r.code) }));
      return section(kind, tree);
    });
    replace(body, ...parts);
  }

  async function load() {
    const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 6, label: '科目を読み込んでいます' }));
    try {
      accounts = await state.load('accounts', { force: true });
      stop();
      if (!disposed) render();
    } catch (err) {
      stop();
      if (!disposed) replace(body, banner({ kind: 'danger', title: '科目を読み込めませんでした', body: err.message }));
    }
  }

  load();
  const cleanup = () => {
    disposed = true;
  };
  cleanup.update = (next) => {
    const nq = next.query.q || '';
    const nall = next.query.all === '1';
    // 打った文字で、もう描いてある。アドレスが追いついただけなら描き直さない（行のフォーカスを失わないため）。
    if (nq === q && nall === showAll) return true;
    q = nq;
    showAll = nall;
    allBox.checked = showAll;
    if (document.activeElement !== search) search.value = q;
    if (accounts.length) render();
    return true;
  };
  return cleanup;
}
