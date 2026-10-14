// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-09 取引先 `#/partners`。名前・読み・登録番号（T+13桁。形式だけを検査）・メモ。行で Enter を押すと直す。
// 仕訳の入力で新しい名前を打つと、サーバーがその場で取引先に足す（ここには、その名前も並ぶ）。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { filterItems } from '../kana.js';
import { createTable } from '../components/table.js';
import { textInput, button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { partnerDialog } from './master-forms.js';

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let partners = [];
  let q = ctx.query.q || '';
  let disposed = false;

  const search = textInput({ type: 'search', value: q, placeholder: '名前・読み・登録番号で探す', 'aria-label': '取引先を探す', dataset: { search: '1' } });
  search.addEventListener('input', () => {
    q = search.value.trim();
    render();
  });
  const commitSearch = () => ctx.navigate(ctx.router.buildHash('/partners', { q: search.value.trim() }), { replace: true });
  search.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e) || e.key !== 'Enter') return;
    e.preventDefault();
    commitSearch();
  });
  search.addEventListener('change', commitSearch);
  const body = h('div', { class: 'masters' });
  root.append(
    h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '取引先'), h('p', { class: 'muted small' }, '取引の登録で新しい名前を打つと、ここに足されます。')), h('div', { class: 'page-head-actions' }, button({ label: '取引先を足す', variant: 'primary', iconName: 'plus', onClick: () => edit(null) }))),
    h('div', { class: 'card masters-tools', role: 'search', 'aria-label': '取引先の絞り込み' }, h('div', { class: 'search-box' }, icon('search', { size: 18 }), search, h('kbd', { 'aria-hidden': 'true' }, '/'))),
    body,
  );

  async function edit(p) {
    const saved = await partnerDialog({ api, partner: p || undefined });
    if (!saved) return;
    toast.success(p ? `「${saved.name}」を保存しました。` : `取引先「${saved.name}」を足しました。`);
    state.invalidate('partners');
    load();
  }

  function render() {
    if (!partners.length) {
      replace(body, h('div', { class: 'card' }, emptyState({ iconName: 'building', title: '取引先はまだありません', body: '取引の登録で取引先の名前を打つと、自動で足されます。先に足しておくこともできます。', actions: [{ label: '取引先を足す', onClick: () => edit(null), primary: true }] })));
      return;
    }
    const items = partners.map((p) => ({ ...p, value: String(p.id), label: p.name, keywords: p.invoice_number || '' }));
    const hits = q ? filterItems(items, q) : items;
    if (!hits.length) {
      replace(body, h('div', { class: 'card' }, emptyState({ iconName: 'search', title: '当てはまる取引先はありません', body: '探す言葉を変えてください。' })));
      return;
    }
    const t = createTable({
      caption: '取引先の一覧（行で Enter を押すと直す）',
      className: 'partners-table',
      rowKey: (p) => p.id,
      rowClass: (p) => (p.enabled === false ? 'is-off' : null),
      onOpen: (p) => edit(partners.find((x) => x.id === p.id)),
      columns: [
        { key: 'name', label: '名前', stackLabel: '', className: 'wrap', render: (p) => [h('span', { class: 'acct-name' }, p.name), p.enabled === false ? h('span', { class: 'badge badge--muted' }, '使っていない') : null] },
        { key: 'reading', label: '読み', render: (p) => p.reading || '' },
        { key: 'invoice_number', label: '登録番号', render: (p) => (p.invoice_number ? h('code', { class: 'mono' }, p.invoice_number) : h('span', { class: 'muted' }, 'なし')) },
        { key: 'note', label: 'メモ', className: 'wrap', render: (p) => p.note || '' },
      ],
      rows: hits,
    });
    replace(body, h('p', { class: 'list-summary' }, h('span', null, `${hits.length}件`)), t.el);
  }

  async function load() {
    const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 5, label: '取引先を読み込んでいます' }));
    try {
      partners = await state.load('partners', { force: true });
      stop();
      if (!disposed) render();
    } catch (err) {
      stop();
      if (!disposed) replace(body, banner({ kind: 'danger', title: '取引先を読み込めませんでした', body: err.message }));
    }
  }

  load();
  const cleanup = () => {
    disposed = true;
  };
  cleanup.update = (next) => {
    const nq = next.query.q || '';
    // 打った文字で、もう描いてある。アドレスが追いついただけなら描き直さない（行のフォーカスを失わないため）。
    if (nq === q) return true;
    q = nq;
    if (document.activeElement !== search) search.value = q;
    if (partners.length) render();
    return true;
  };
  return cleanup;
}
