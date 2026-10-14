// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 仕訳帳の絞り込み。条件はアドレスの ?以降に持つ（戻る操作で前の条件に戻れる。SC-05）。
// 期間・科目・口座・金額の範囲・文字列・状態・種別。前の月と次の月は [ と ] でも移れる。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatMonth } from '../format.js';
import { monthRange, shiftMonth, todayIso, parseAmountExpr } from '../input-parse.js';
import { field, selectInput, textInput, iconButton, button } from '../components/fields.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { accountItems, walletItems } from './entry-options.js';

export const PAGE_SIZE = 50;
const KEYS = ['from', 'to', 'account', 'wallet', 'q', 'kind', 'state', 'min', 'max', 'page'];
const isDate = (s) => /^\d{4}-\d{2}-\d{2}$/.test(s || '');

/** アドレスの条件を読む（読めない値は捨てる）。 */
export function readFilters(query) {
  const f = {};
  for (const k of KEYS) if (query[k]) f[k] = query[k];
  if (f.from && !isDate(f.from)) delete f.from;
  if (f.to && !isDate(f.to)) delete f.to;
  if (!['active', 'voided', 'all'].includes(f.state)) f.state = 'all';
  if (!['expense', 'income', 'transfer', 'journal', 'opening'].includes(f.kind)) delete f.kind;
  for (const k of ['min', 'max']) if (f[k] && !/^\d+$/.test(f[k])) delete f[k];
  const page = Number(f.page);
  f.page = Number.isInteger(page) && page > 1 ? page : 1;
  return f;
}

/** 条件をアドレスの形に（既定の値は書かない）。 */
export function toQuery(f) {
  const q = { ...f };
  if (q.state === 'all') delete q.state;
  if (q.page === 1) delete q.page;
  return q;
}

/** サーバーへの問い合わせ（GET /entries）。 */
export function toApiParams(f) {
  const { page, ...rest } = f;
  return { ...rest, limit: PAGE_SIZE, offset: (page - 1) * PAGE_SIZE };
}

/** 期間がちょうど1か月なら、その月（YYYY-MM）。 */
export function exactMonth(f) {
  if (!f.from || !f.to) return null;
  const ym = f.from.slice(0, 7);
  const r = monthRange(ym);
  return r.from === f.from && r.to === f.to ? ym : null;
}

/**
 * 前の月・次の月へずらした条件。期間がちょうど1か月なら、その前後の月。
 * そうでなければ、最初の1回は「今月」（期間の始めがあれば、その月）に合わせる。
 */
export function shiftPeriod(f, n) {
  const m = exactMonth(f);
  const ym = m ? shiftMonth(m, n) : (f.from || todayIso()).slice(0, 7);
  const r = monthRange(ym);
  return { ...f, from: r.from, to: r.to, page: 1 };
}

/** 期間の見出し。 */
export function periodLabel(f) {
  const m = exactMonth(f);
  if (m) return formatMonth(m);
  if (!f.from && !f.to) return 'すべての期間';
  return `${f.from || '最初'} 〜 ${f.to || '最後'}`;
}

const KIND_OPTIONS = [
  { value: '', label: 'すべて' },
  { value: 'expense', label: '支出' },
  { value: 'income', label: '収入' },
  { value: 'transfer', label: '振替' },
  { value: 'journal', label: '仕訳の形' },
  { value: 'opening', label: '期首残高' },
];
const STATE_OPTIONS = [
  { value: 'all', label: '取消済みも出す' },
  { value: 'active', label: '有効だけ（取消済みを隠す）' },
  { value: 'voided', label: '取消済みだけ' },
];

/**
 * 絞り込みの入れ物を作る。条件が決まったら apply(新しい条件) を呼ぶ。
 * @param {{ filters: object, accounts: any[], apply: (f: object) => void }} opts
 */
export function createFilters(opts) {
  let f = opts.filters;
  const apply = (patch) => opts.apply({ ...f, ...patch, page: 1 });

  const label = h('span', { class: 'period-label', 'aria-live': 'polite' });
  const prev = iconButton({ iconName: 'chevron-left', label: '前の月（[ キー）', onClick: () => opts.apply(shiftPeriod(f, -1)) });
  const next = iconButton({ iconName: 'chevron-right', label: '次の月（] キー）', onClick: () => opts.apply(shiftPeriod(f, 1)) });
  const allBtn = button({ label: 'すべての期間', small: true, variant: 'ghost', onClick: () => apply({ from: undefined, to: undefined }) });

  const search = textInput({ type: 'search', placeholder: '摘要・取引先・金額など', 'aria-label': '文字で探す', dataset: { search: '1' } });
  const commitSearch = () => {
    if ((search.value.trim() || undefined) !== f.q) apply({ q: search.value.trim() || undefined });
  };
  search.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      commitSearch();
    }
  });
  search.addEventListener('change', commitSearch);
  const searchBox = h('div', { class: 'search-box' }, icon('search', { size: 18 }), search, h('kbd', { 'aria-hidden': 'true' }, '/'));

  const fromIn = createDateInput({ onChange: (v) => v !== f.from && apply({ from: v || undefined }) });
  const toIn = createDateInput({ onChange: (v) => v !== f.to && apply({ to: v || undefined }) });
  const accountCb = createCombobox({ items: accountItems(opts.accounts, 'expense'), listLabel: '科目の候補', placeholder: 'すべての科目', onSelect: (item) => (item ? item.value : undefined) !== f.account && apply({ account: item ? item.value : undefined }) });
  const walletSel = selectInput({}, [{ value: '', label: 'すべての口座' }, ...walletItems(opts.accounts).map((w) => ({ value: w.value, label: w.label }))]);
  walletSel.addEventListener('change', () => apply({ wallet: walletSel.value || undefined }));
  const kindSel = selectInput({}, KIND_OPTIONS);
  kindSel.addEventListener('change', () => apply({ kind: kindSel.value || undefined }));
  const stateSel = selectInput({}, STATE_OPTIONS);
  stateSel.addEventListener('change', () => apply({ state: stateSel.value }));
  const amountIn = (key) => {
    const el = textInput({ inputmode: 'numeric', class: 'input--num', placeholder: key === 'min' ? '下限なし' : '上限なし' });
    el.addEventListener('change', () => {
      const r = parseAmountExpr(el.value);
      apply({ [key]: el.value.trim() && r.ok ? String(r.value) : undefined });
    });
    return el;
  };
  const minIn = amountIn('min');
  const maxIn = amountIn('max');
  const clearBtn = button({ label: '条件を消す', small: true, iconName: 'close', onClick: () => opts.apply({ state: 'all', page: 1 }) });
  const count = h('span', { class: 'badge badge--muted filter-count', hidden: true });

  const more = h(
    'details',
    { class: 'filter-more' },
    h('summary', null, icon('filter', { size: 18 }), h('span', null, 'くわしい条件'), count),
    h(
      'div',
      { class: 'filter-grid' },
      field({ label: '期間の始め', control: fromIn.el }).el,
      field({ label: '期間の終わり', control: toIn.el }).el,
      field({ label: '科目', control: accountCb.el, labelFor: accountCb.input }).el,
      field({ label: '口座', control: walletSel }).el,
      field({ label: '種別', control: kindSel }).el,
      field({ label: '状態', control: stateSel }).el,
      field({ label: '金額の下限', control: h('div', { class: 'affix' }, minIn, h('span', { class: 'affix-text' }, '円')), labelFor: minIn }).el,
      field({ label: '金額の上限', control: h('div', { class: 'affix' }, maxIn, h('span', { class: 'affix-text' }, '円')), labelFor: maxIn }).el,
    ),
    h('div', { class: 'filter-actions' }, clearBtn),
  );

  const el = h(
    'div',
    { class: 'card filters', role: 'search', 'aria-label': '仕訳の絞り込み' },
    h('div', { class: 'filter-top' }, h('div', { class: 'period' }, prev, label, next, allBtn), searchBox),
    more,
  );

  // くわしい条件が付いた状態で開いたときは、最初の1回だけ中身を開いて見せる。
  let autoOpened = false;
  function sync(next) {
    f = next;
    label.textContent = periodLabel(f);
    allBtn.hidden = !f.from && !f.to;
    if (document.activeElement !== search) search.value = f.q || '';
    fromIn.setValue(f.from || null);
    toIn.setValue(f.to || null);
    accountCb.setValue(f.account || null);
    walletSel.value = f.wallet || '';
    kindSel.value = f.kind || '';
    stateSel.value = f.state || 'all';
    minIn.value = f.min || '';
    maxIn.value = f.max || '';
    const n = ['from', 'to', 'account', 'wallet', 'kind', 'min', 'max'].filter((k) => f[k]).length + (f.state !== 'all' ? 1 : 0);
    count.hidden = !n;
    count.textContent = n ? `${n}件の条件` : '';
    if (n && !more.open && !autoOpened) {
      more.open = true;
      autoOpened = true;
    }
  }
  sync(f);

  return { el, sync, focusSearch: () => search.focus(), destroy: () => accountCb.destroy() };
}
