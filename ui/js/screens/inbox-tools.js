// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の絞り込みの帯（SC-03）: 表示する明細のタブ、口座、文字で探す欄。絞り込みの値はアドレスに持つ（inbox.js）。
// 文字で探す欄は、Enter で探す（打つそばからは絞らない）。それが分かるよう、欄にいる間は「Enter で探します」と出す。
// Esc で欄から抜ける。抜けないと、続けて押した g h（ホームへ）が、欄の文字として入った。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { createTabs } from '../components/tabs.js';
import { selectInput, textInput } from '../components/fields.js';
import { walletItems } from './entry-options.js';

const HINT_ID = 'inbox-search-hint';

/**
 * @param {{ filters: () => { status: string, wallet: string, q: string }, apply: (patch: object) => void }} o
 */
export function createInboxTools(o) {
  const tabs = createTabs({ label: '表示する明細', tabs: [{ id: 'draft', label: '未確認' }, { id: 'ignored', label: '無視した行' }], selected: o.filters().status, onChange: (id) => o.apply({ status: id }) });
  const walletSel = selectInput({ 'aria-label': '口座で絞り込む', class: 'inbox-wallet' }, [{ value: '', label: 'すべての口座' }]);
  walletSel.addEventListener('change', () => o.apply({ wallet: walletSel.value }));
  const search = textInput({ type: 'search', placeholder: '摘要・取引先・金額で探す', 'aria-label': '明細を文字で探す', 'aria-describedby': HINT_ID, dataset: { search: '1' } });
  const commit = () => search.value.trim() !== o.filters().q && o.apply({ q: search.value.trim() });
  search.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      commit();
    } else if (e.key === 'Escape') {
      // 欄の文字は残す（ブラウザの標準では、検索の欄の Esc は文字を消す）。欄を離れると、ほかの欄と同じく探す（change）。
      e.preventDefault();
      search.blur();
    }
  });
  search.addEventListener('change', commit);
  const box = h('div', { class: 'search-box inbox-search' }, icon('search', { size: 18 }), search, h('kbd', { 'aria-hidden': 'true' }, '/'), h('span', { class: 'search-hint', id: HINT_ID }, 'Enter で探します'));
  const el = h('div', { class: 'card inbox-tools', role: 'search', 'aria-label': '未確認の絞り込み' }, tabs.el, h('div', { class: 'inbox-filters' }, walletSel, box));
  return {
    el,
    panel: tabs.panel,
    /** 件数をタブの名前に入れる。 */
    paintCounts(counts) {
      tabs.el.querySelector('[data-tab="draft"]').textContent = `未確認 ${counts.draft}件`;
      tabs.el.querySelector('[data-tab="ignored"]').textContent = `無視した行 ${counts.ignored}件`;
    },
    /** アドレスの絞り込みを、欄に映す（打っている途中の検索の欄は書き換えない）。 */
    sync(f) {
      tabs.select(f.status);
      walletSel.value = f.wallet;
      if (document.activeElement !== search) search.value = f.q;
    },
    setWallets(accounts) {
      walletSel.replaceChildren(h('option', { value: '' }, 'すべての口座'), ...walletItems(accounts).map((w) => h('option', { value: w.value }, w.label)));
    },
    /** 「未確認」のタブへフォーカスを移す（無視した行を全部戻したあと）。 */
    focusDraftTab() {
      tabs.el.querySelector('[data-tab="draft"]').focus();
    },
  };
}
