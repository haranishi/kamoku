// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-07 口座 `#/wallets`。口座ごとに名前・型・残高（カードは「未払い額」）。口座から、その口座の総勘定元帳と、
// 未確認の件数（その口座で絞った未確認）へ飛べる。新しい口座は「現金」「預金」「クレジットカード」「電子マネー」から作る。

import { h, replace } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { walletDialog, WALLET_TYPES } from './master-forms.js';

const WALLET_ICON = { cash: 'wallet', bank: 'building', card: 'card', ewallet: 'wallet' };
const typeLabel = (t) => (WALLET_TYPES.find((x) => x.value === t) || { label: '' }).label;

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let disposed = false;
  const body = h('div', { class: 'masters' });
  const addBtn = button({ label: '口座を足す', variant: 'primary', iconName: 'plus', onClick: () => add() });
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '口座'), h('p', { class: 'muted small' }, 'お金の置き場所です。取引の登録と、明細の取込で使います。')), h('div', { class: 'page-head-actions' }, addBtn)), body);

  async function add() {
    const saved = await walletDialog({ api });
    if (!saved) return;
    toast.success(`口座「${saved.name}」を足しました。`);
    state.invalidate('accounts');
    load();
  }

  async function edit(w) {
    const saved = await walletDialog({ api, wallet: w });
    if (!saved) return;
    toast.success(`口座「${saved.name}」を保存しました。`);
    state.invalidate('accounts');
    load();
  }

  function item(w, bal, drafts) {
    const card = w.wallet.type === 'card';
    const amount = bal ? bal.balance : null;
    return h(
      'li',
      { class: ['card', 'wallet-item', !w.enabled && 'is-off'] },
      h('span', { class: 'row-icon wallet-icon' }, icon(WALLET_ICON[w.wallet.type] || 'wallet')),
      h(
        'div',
        { class: 'wallet-main' },
        h('p', { class: 'wallet-name' }, w.name, w.enabled ? null : h('span', { class: 'badge badge--muted' }, '使っていない')),
        h('p', { class: 'muted small' }, [typeLabel(w.wallet.type), w.wallet.institution].filter(Boolean).join('・')),
        h(
          'div',
          { class: 'wallet-links' },
          h('a', { href: `#/reports/ledger?account=${encodeURIComponent(w.code)}` }, icon('book', { size: 16 }), 'この口座の元帳'),
          h('a', { href: `#/inbox?wallet=${encodeURIComponent(w.code)}` }, icon('inbox', { size: 16 }), drafts === null ? '未確認' : `未確認 ${drafts}件`),
          button({ label: '直す', small: true, variant: 'ghost', iconName: 'edit', 'aria-label': `「${w.name}」を直す`, onClick: () => edit(w) }),
        ),
      ),
      h('div', { class: 'wallet-balance' }, h('span', { class: 'muted small' }, card ? '未払い額' : '残高'), h('span', { class: 'wallet-amount' }, amount === null ? '—' : [h('span', { class: 'num' }, formatYen(amount)), h('span', { class: 'unit' }, '円')])),
    );
  }

  async function load() {
    const stop = delayedSkeleton(body, () => skeletonBlock({ lines: 4, label: '口座を読み込んでいます' }));
    try {
      const [accounts, balances] = await Promise.all([state.load('accounts', { force: true }), api.get('/wallets')]);
      const wallets = accounts.filter((a) => a.wallet).sort((a, b) => a.sort - b.sort);
      const counts = await Promise.all(wallets.map((w) => api.get('/drafts', { status: 'draft', wallet: w.code, limit: 1 }).then((r) => r.total).catch(() => null)));
      stop();
      if (disposed) return;
      const balOf = new Map(balances.items.map((b) => [b.code, b]));
      if (!wallets.length) {
        replace(body, h('div', { class: 'card' }, emptyState({ iconName: 'wallet', title: '口座がまだありません', body: '現金、預金、クレジットカード、電子マネーから、使っている口座を足してください。', actions: [{ label: '口座を足す', onClick: add, primary: true }] })));
        return;
      }
      const on = wallets.filter((w) => w.enabled);
      const off = wallets.filter((w) => !w.enabled);
      replace(
        body,
        h(
          'dl',
          { class: 'card totals wallet-totals' },
          h('div', null, h('dt', null, '現金と預金の合計'), h('dd', null, h('span', { class: 'num' }, formatYen(balances.cash_total)), '円')),
          h('div', null, h('dt', null, 'カードの未払いの合計'), h('dd', null, h('span', { class: 'num' }, formatYen(balances.owed_total)), '円')),
        ),
        h('ul', { class: 'wallet-list', 'aria-label': '使っている口座' }, on.map((w) => item(w, balOf.get(w.code), counts[wallets.indexOf(w)]))),
        off.length ? h('section', { class: 'masters-off', 'aria-labelledby': 'off-title' }, h('h2', { id: 'off-title', class: 'card-title' }, '使っていない口座'), h('ul', { class: 'wallet-list' }, off.map((w) => item(w, null, counts[wallets.indexOf(w)])))) : null,
      );
    } catch (err) {
      stop();
      if (!disposed) replace(body, banner({ kind: 'danger', title: '口座を読み込めませんでした', body: err.message }));
    }
  }

  load();
  return () => {
    disposed = true;
  };
}
