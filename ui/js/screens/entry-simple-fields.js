// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の入力欄の組み立て（DOMを作る部分だけ）。並びは keyboard.md のとおり:
// 種別、日付、金額、科目（振替は移動先の口座）、口座、税区分、取引先、摘要、割合。
// 値の決め方とキー操作は entry-simple.js が持つ。ここは、欄が変わったら on の関数を呼ぶだけ。

import { h, uid } from '../dom.js';
import { normalizeWidth } from '../input-parse.js';
import { field, selectInput, textInput } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { createAmountInput } from '../components/amount-input.js';
import { taxOptions } from '../account-defaults.js';
import { accountItems, walletItems, partnerItems } from './entry-options.js';

export const KINDS = [
  { value: 'expense', label: '支出', key: '1' },
  { value: 'income', label: '収入', key: '2' },
  { value: 'transfer', label: '振替', key: '3' },
];
export const WALLET_LABEL = { expense: '支払った口座', income: '入金先の口座', transfer: '移動元の口座' };

/**
 * @param {{ accounts: any[], partners: any[], s: object,
 *           on: { changed: () => void, setKind: (k: string) => void, setAccount: (code: string|null) => void,
 *                 syncNotes: () => void, openDetail: () => void, validatePct: () => boolean } }} p
 *   s は入力の状態（entry-simple.js が持つ）。欄が変わると、ここで書き換えて on.changed() を呼ぶ。
 */
export function buildSimpleFields({ accounts, partners, s, on }) {
  const u = {};
  u.kindSeg = createSegmented({ legend: '種別', options: KINDS, value: s.kind, onChange: (k) => on.setKind(k) });

  u.transitionalNote = h('p', { class: 'field-hint field-note', id: uid('note'), hidden: true }, '課税仕入れの日（商品の引渡し・役務の提供が完了した日）にしてください。経過措置の割合は、この日付で決まります。');
  u.dateIn = createDateInput({ value: s.date, onChange: (v) => { s.date = v; on.changed(); }, onError: (m) => u.dateF.setError(m) });
  u.dateF = field({ label: '日付', control: u.dateIn.el, required: true, after: u.transitionalNote });

  const amountNote = h('p', { class: 'field-hint field-note', hidden: true });
  u.amountIn = createAmountInput({
    value: s.amount,
    onChange: (n) => { s.amount = n; on.changed(); },
    onError: (m) => u.amountF.setError(m),
    onNote: (t) => { amountNote.textContent = t; amountNote.hidden = !t; },
  });
  u.amountF = field({ label: '金額', control: h('div', { class: 'affix' }, u.amountIn.el, h('span', { class: 'affix-text' }, '円')), labelFor: u.amountIn.el, required: true, after: amountNote });

  u.accountCb = createCombobox({ items: accountItems(accounts, s.kind), value: s.account, listLabel: '科目の候補', placeholder: '例: つうしん、通信費', onSelect: (item) => on.setAccount(item ? item.value : null) });
  u.accountF = field({ label: '科目', control: u.accountCb.el, labelFor: u.accountCb.input, required: true });
  u.toWalletCb = createCombobox({ items: walletItems(accounts), value: s.to_wallet, listLabel: '口座の候補', onSelect: (item) => { s.to_wallet = item ? item.value : null; on.changed(); } });
  u.toWalletF = field({ label: '移動先の口座', control: u.toWalletCb.el, labelFor: u.toWalletCb.input, required: true });
  u.walletCb = createCombobox({ items: walletItems(accounts), value: s.wallet, listLabel: '口座の候補', onSelect: (item) => { s.wallet = item ? item.value : null; on.changed(); } });
  u.walletF = field({ label: WALLET_LABEL[s.kind], control: u.walletCb.el, labelFor: u.walletCb.input, required: true });

  u.taxSel = selectInput({ value: s.tax_code || '' }, taxOptions(s.kind));
  u.taxSel.addEventListener('change', () => { s.tax_code = u.taxSel.value || null; on.syncNotes(); on.changed(); });
  u.taxF = field({ label: '税区分', control: u.taxSel });
  u.taxF.el.id = uid('taxf');
  u.detailBtn = h('button', { type: 'button', class: 'btn btn--ghost btn--sm detail-toggle', 'aria-expanded': 'false', 'aria-controls': u.taxF.el.id }, '詳細: 税区分を出す');
  u.detailBtn.addEventListener('click', () => on.openDetail());
  u.detailCell = h('div', { class: 'field detail-cell' }, h('span', { class: 'field-label' }, '税区分'), u.detailBtn, h('p', { class: 'field-hint' }, '免税事業者の設定なので、ふだんは隠しています。'));
  u.lockCell = h('div', { class: 'field detail-cell' }, h('span', { class: 'field-label' }, '税区分と割合'), h('p', { class: 'field-hint' }, '相手が口座の取引（カードの引き落としなど）なので、税区分は対象外、割合は100%のままにします。'));

  u.partnerCb = createCombobox({
    items: partnerItems(partners),
    allowFree: true,
    value: s.partner,
    listLabel: '取引先の候補',
    placeholder: '例: サンプル通信',
    onInput: () => { s.partner = u.partnerCb.value; on.changed(); },
    onSelect: () => { s.partner = u.partnerCb.value; on.changed(); },
  });
  u.partnerF = field({ label: '取引先', control: u.partnerCb.el, labelFor: u.partnerCb.input, hint: '候補に無い名前も入れられます。' });
  u.descIn = textInput({ value: s.description, placeholder: '例: 10月分 携帯電話', enterkeyhint: 'next' });
  u.descIn.addEventListener('input', () => { s.description = u.descIn.value; on.changed(); });
  u.descF = field({ label: '摘要', control: u.descIn, className: 'span-2' });

  u.pctIn = textInput({ value: String(s.business_pct), inputmode: 'numeric', class: 'input--num pct-input', enterkeyhint: 'done' });
  u.ownDraw = h('span', { class: 'pct-out', 'aria-live': 'polite' });
  u.pctIn.addEventListener('input', () => {
    const n = Number(normalizeWidth(u.pctIn.value).trim());
    if (Number.isInteger(n) && n >= 0 && n <= 100 && u.pctIn.value.trim() !== '') {
      s.business_pct = n;
      u.pctF.setError('');
      on.changed();
    }
  });
  u.pctIn.addEventListener('blur', () => on.validatePct());
  u.pctF = field({ label: '事業の割合', control: h('div', { class: 'affix' }, u.pctIn, h('span', { class: 'affix-text' }, '%'), u.ownDraw), labelFor: u.pctIn, hint: '仕事に使った分の割合。残りは事業主貸に入ります。', className: 'span-2' });

  u.grid = h('div', { class: 'entry-grid' }, u.dateF.el, u.amountF.el, u.accountF.el, u.toWalletF.el, u.walletF.el, u.taxF.el, u.detailCell, u.lockCell, u.partnerF.el, u.descF.el, u.pctF.el);
  return u;
}
