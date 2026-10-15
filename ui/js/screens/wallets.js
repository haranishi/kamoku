// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// SC-07 口座 `#/wallets`。口座ごとに名前・型・残高（カードは「未払い額」）。口座から、その口座の総勘定元帳と、
// 未確認の件数（その口座で絞った未確認）へ飛べる。新しい口座は、種類（「現金」「預金」「クレジットカード」「電子マネー」）を選んで追加する。

import { h, replace, srOnly } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { button } from '../components/fields.js';
import { banner } from '../components/banner.js';
import { emptyState } from '../components/empty.js';
import { skeletonBlock, delayedSkeleton } from '../components/skeleton.js';
import { money, emptyValue } from '../components/money.js';
import { walletDialog, WALLET_TYPES } from './master-forms.js';
import { phrase } from '../components/phrase.js';

const WALLET_ICON = { cash: 'wallet', bank: 'building', card: 'card', ewallet: 'wallet' };
const typeLabel = (t) => (WALLET_TYPES.find((x) => x.value === t) || { label: '' }).label;

/** 上の合計の1つ。桁の多い金額でも枠に収まるよう、文字の数を CSS に渡す（screens-masters.css の .wallet-totals dd）。 */
function total(label, n) {
  const dd = h('dd', null, money(n));
  dd.style.setProperty('--chars', String(formatYen(n).length));
  return h('div', null, h('dt', null, label), dd);
}

/** @param {HTMLElement} root */
export function mount(root, ctx) {
  const { api, state, toast } = ctx;
  let disposed = false;
  const body = h('div', { class: 'masters' });
  const addBtn = button({ label: '口座を追加', variant: 'primary', iconName: 'plus', onClick: () => add() });
  root.append(h('div', { class: 'page-head' }, h('div', { class: 'page-head-text' }, h('h1', { tabindex: '-1' }, '口座'), h('p', { class: 'muted small' }, phrase('お金の置き場所です。取引の登録と、明細の取込で使います。'))), h('div', { class: 'page-head-actions' }, addBtn)), body);

  async function add() {
    const saved = await walletDialog({ api });
    if (!saved) return;
    toast.success(`口座「${saved.name}」を追加しました。`);
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

  /**
   * 未確認の件数。1件以上なら藍の札にして、手を付ける口座が目に入るようにする（0件と同じ見た目で埋もれた）。
   * 読み上げの名前には口座の名前を足す（口座ごとに同じ「未確認 0件」のリンクが並び、区別できなかった）。
   */
  function draftsLink(w, drafts) {
    const href = `#/inbox?wallet=${encodeURIComponent(w.code)}`;
    const count = drafts === null ? null : drafts > 0 ? h('span', { class: 'badge badge--info' }, `${formatYen(drafts)}件`) : h('span', { class: 'muted' }, '0件');
    return h('a', { class: 'btn btn--sm wallet-link', href }, icon('inbox', { size: 16 }), h('span', null, '未確認'), count, srOnly(`（${w.name}）`));
  }

  function item(w, bal, drafts) {
    const card = w.wallet.type === 'card';
    const amount = bal ? bal.balance : null;
    // 名前の下の補足は、型と金融機関。名前と同じ言葉（「現金」の下に「現金」）なら出さない。
    const note = [typeLabel(w.wallet.type), w.wallet.institution].filter((x) => x && x !== w.name).join('・');
    // 操作の並びは、カードの下の段に置く（名前の列の中では幅が足りず、「直す」だけが2行目に落ちた）。
    // 狭い幅（640px未満）では「この口座の」を省いて「元帳」と見せる。読み上げの名前は、見える文字に口座の名前を足す
    // （「この口座の元帳（現金）」。同じ名前のリンクが口座の数だけ並び、どの口座のものか分からなかった）。
    return h(
      'li',
      { class: ['card', 'wallet-item', !w.enabled && 'is-off'] },
      h('span', { class: 'row-icon wallet-icon' }, icon(WALLET_ICON[w.wallet.type] || 'wallet')),
      h(
        'div',
        { class: 'wallet-main' },
        h('p', { class: 'wallet-name' }, h('span', { class: 'keep' }, w.name), w.enabled ? null : h('span', { class: 'badge badge--muted' }, '使っていない')),
        note ? h('p', { class: 'muted small' }, note) : null,
      ),
      h('div', { class: 'wallet-balance' }, h('span', { class: 'muted small' }, card ? '未払い額' : '残高'), h('span', { class: 'wallet-amount' }, amount === null ? emptyValue() : money(amount))),
      h(
        'div',
        { class: 'wallet-links' },
        h('a', { class: 'btn btn--sm wallet-link', href: `#/reports/ledger?account=${encodeURIComponent(w.code)}` }, icon('book', { size: 16 }), h('span', null, h('span', { class: 'wallet-link-long' }, 'この口座の'), '元帳'), srOnly(`（${w.name}）`)),
        draftsLink(w, drafts),
        button({ label: '直す', small: true, iconName: 'edit', className: 'wallet-link', 'aria-label': `「${w.name}」を直す`, onClick: () => edit(w) }),
      ),
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
        replace(body, h('div', { class: 'card' }, emptyState({ iconName: 'wallet', title: '口座がまだありません', body: '現金、預金、クレジットカード、電子マネーから、使っている口座を追加してください。', actions: [{ label: '口座を追加', onClick: add, primary: true }] })));
        return;
      }
      const on = wallets.filter((w) => w.enabled);
      const off = wallets.filter((w) => !w.enabled);
      replace(
        body,
        h(
          'dl',
          { class: 'card totals wallet-totals' },
          total('現金と預金の合計', balances.cash_total),
          total('カードの未払いの合計', balances.owed_total),
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
