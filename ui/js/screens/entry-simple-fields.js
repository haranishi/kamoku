// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の入力欄の組み立て（DOMを作る部分だけ）。並びは keyboard.md のとおり:
// 種別、日付、金額、科目（振替は移動先の口座）、口座、税区分、取引先、摘要、割合。
// 値の決め方とキー操作は entry-simple.js が持つ。ここは、欄が変わったら on の関数を呼ぶだけ。

import { h, uid, addDescribedBy, removeDescribedBy, selectAllOnFocus } from '../dom.js';
import { field, selectInput, textInput } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { createAmountInput } from '../components/amount-input.js';
import { taxOptions } from '../account-defaults.js';
import { accountItems, walletItems, partnerItems } from './entry-options.js';
import { parsePct } from './entry-hints.js';
import { phrase } from '../components/phrase.js';

export const KINDS = [
  { value: 'expense', label: '支出', key: '1' },
  { value: 'income', label: '収入', key: '2' },
  { value: 'transfer', label: '振替', key: '3' },
];
export const WALLET_LABEL = { expense: '支払った口座', income: '入金先の口座', transfer: '移動元の口座' };

/** 候補つきの欄で、打った文字に合う候補が無いときの一言（一覧は閉じる）。 */
export const NO_ACCOUNT = '合う科目がありません。ひらがなの読みか、科目の名前で探せます。';
export const NO_WALLET = '合う口座がありません。ひらがなの読みか、口座の名前で探せます。';
/** 取引先の候補の一覧の、最後の行（打った名前を新しい取引先として使う）。 */
export const newPartnerLabel = (name) => `「${name}」を新しい取引先として足す`;

/**
 * 欄の下の一言（科目の既定の割合の出どころ、税区分の気づき、新しい取引先）。出ている間だけ、欄の説明として読み上げに結ぶ。
 * @param {() => HTMLElement} control 結ぶ先の入力欄
 */
export function fieldNote(control) {
  const el = h('p', { class: 'field-hint field-note', id: uid('note'), hidden: true });
  return {
    el,
    /** plain が真なら、文を塊に分けない（「新しい取引先: 名前」の空白が、塊の端で消えないように）。 */
    set(text, { plain = false } = {}) {
      if (el.textContent === text && el.hidden === !text) return;
      el.replaceChildren(text ? (plain ? text : phrase(text)) : '');
      el.hidden = !text;
      if (text) addDescribedBy(control(), el.id);
      else removeDescribedBy(control(), el.id);
    },
  };
}

/**
 * @param {{ accounts: any[], partners: any[], s: object,
 *           on: { changed: () => void, setKind: (k: string) => void, setAccount: (code: string|null) => void,
 *                 syncNotes: () => void, openDetail: () => void, validatePct: () => boolean, dateChanged: (iso: string|null) => void } }} p
 *   s は入力の状態（entry-simple.js が持つ）。欄が変わると、ここで書き換えて on.changed() を呼ぶ。
 */
export function buildSimpleFields({ accounts, partners, s, on }) {
  const u = {};
  u.kindSeg = createSegmented({ legend: '種別', options: KINDS, value: s.kind, onChange: (k) => on.setKind(k) });

  u.transitionalNote = h('p', { class: 'field-hint field-note', id: uid('note'), hidden: true }, phrase('課税仕入れの日（商品の引渡し・役務の提供が完了した日）にしてください。経過措置の割合は、この日付で決まります。'));
  u.dateIn = createDateInput({ value: s.date, onChange: (v) => { s.date = v; on.dateChanged(v); on.changed(); }, onError: (m) => u.dateF.setError(m) });
  u.dateF = field({ label: '日付', control: u.dateIn.el, required: true, after: u.transitionalNote });

  const amountNote = h('p', { class: 'field-hint field-note', hidden: true });
  u.amountIn = createAmountInput({
    value: s.amount,
    onChange: (n) => { s.amount = n; on.changed(); },
    onError: (m) => u.amountF.setError(m),
    onNote: (t) => { amountNote.textContent = t; amountNote.hidden = !t; },
  });
  u.amountF = field({ label: '金額', control: h('div', { class: 'affix' }, u.amountIn.el, h('span', { class: 'affix-text' }, '円')), labelFor: u.amountIn.el, required: true, after: amountNote });

  // 候補を選んだら、その欄の誤りの表示（赤枠・文・aria-invalid）をすぐ消す。保存を押し直すまで残さない。
  u.accountCb = createCombobox({ items: accountItems(accounts, s.kind, { forEntry: true, keep: [s.account] }), value: s.account, listLabel: '科目の候補', placeholder: '例: つうしん、通信費', noMatch: NO_ACCOUNT, enterNeedsMatch: true, onSelect: (item) => { if (item) u.accountF.setError(''); on.setAccount(item ? item.value : null); } });
  u.accountF = field({ label: '科目', control: u.accountCb.el, labelFor: u.accountCb.input, required: true });
  u.toWalletCb = createCombobox({ items: walletItems(accounts), value: s.to_wallet, listLabel: '口座の候補', noMatch: NO_WALLET, enterNeedsMatch: true, onSelect: (item) => { if (item) u.toWalletF.setError(''); s.to_wallet = item ? item.value : null; on.changed(); } });
  u.toWalletF = field({ label: '移動先の口座', control: u.toWalletCb.el, labelFor: u.toWalletCb.input, required: true });
  u.walletCb = createCombobox({ items: walletItems(accounts), value: s.wallet, listLabel: '口座の候補', noMatch: NO_WALLET, enterNeedsMatch: true, onSelect: (item) => { if (item) u.walletF.setError(''); s.wallet = item ? item.value : null; on.changed(); } });
  u.walletF = field({ label: WALLET_LABEL[s.kind], control: u.walletCb.el, labelFor: u.walletCb.input, required: true });

  u.taxSel = selectInput({ value: s.tax_code || '' }, taxOptions(s.kind));
  u.taxSel.addEventListener('change', () => { s.tax_code = u.taxSel.value || null; on.syncNotes(); on.changed(); });
  u.taxNote = fieldNote(() => u.taxSel);
  u.taxF = field({ label: '税区分', control: u.taxSel, after: u.taxNote.el });
  u.taxF.el.id = uid('taxf');
  u.detailBtn = h('button', { type: 'button', class: 'btn btn--ghost btn--sm detail-toggle', 'aria-expanded': 'false', 'aria-controls': u.taxF.el.id }, '詳細: 税区分を出す');
  u.detailBtn.addEventListener('click', () => on.openDetail());
  u.detailCell = h('div', { class: 'field detail-cell' }, h('span', { class: 'field-label' }, '税区分'), u.detailBtn, h('p', { class: 'field-hint' }, phrase('免税事業者の設定なので、ふだんは隠しています。')));
  u.lockCell = h('div', { class: 'field detail-cell' }, h('span', { class: 'field-label' }, '税区分と割合'), h('p', { class: 'field-hint' }, phrase('相手が口座の取引（カードの引き落としなど）なので、税区分は対象外、割合は100%のままにします。')));

  // 取引先: 科目の欄と同じく、Enterで先頭の候補を選ぶ。打った名前は、一覧の最後の行を選んだときだけ新しい取引先になる。
  u.partnerCb = createCombobox({
    items: partnerItems(partners),
    allowFree: true,
    createLabel: newPartnerLabel,
    value: s.partner,
    listLabel: '取引先の候補',
    placeholder: '例: サンプル通信',
    onInput: () => {
      s.partner = u.partnerCb.value;
      if (!s.partner) u.partnerF.setError('');
      on.changed();
    },
    onSelect: () => {
      s.partner = u.partnerCb.value;
      u.partnerF.setError('');
      on.changed();
    },
  });
  u.partnerNote = fieldNote(() => u.partnerCb.input);
  u.partnerF = field({ label: '取引先', control: u.partnerCb.el, labelFor: u.partnerCb.input, after: u.partnerNote.el, hint: phrase('新しい名前は、一覧の最後の行で足せます。') });
  u.descIn = textInput({ value: s.description, placeholder: '例: 10月分 携帯電話', enterkeyhint: 'next' });
  u.descIn.addEventListener('input', () => { s.description = u.descIn.value; on.changed(); });
  u.descF = field({ label: '摘要', control: u.descIn, className: 'span-2' });

  u.pctIn = textInput({ value: String(s.business_pct), inputmode: 'numeric', class: 'input--num pct-input', enterkeyhint: 'done' });
  u.ownDraw = h('span', { class: 'pct-out', 'aria-live': 'polite' });
  // 範囲外の値（150、-10、abc）は割合にしない。プレビューは、前の割合の仕訳の代わりに、誤りを出す（entry-simple.js）。
  // 入ったら全体を選ぶ。Enterで入ると文字が選ばれず、既定の「30」に「70」が足されて「3070」になった（2026-10-15 の体験の評価）。
  selectAllOnFocus(u.pctIn);
  u.pctIn.addEventListener('input', () => {
    const n = parsePct(u.pctIn.value);
    if (n !== null) {
      s.business_pct = n;
      u.pctF.setError('');
    } else if (u.pctIn.value.trim()) {
      // 打っている途中でも、範囲外になった時点で欄の下に出す（空のときは、欄を離れるまで待つ）。
      on.validatePct();
    }
    on.changed();
  });
  u.pctIn.addEventListener('blur', () => on.validatePct());
  u.pctNote = fieldNote(() => u.pctIn);
  u.pctF = field({ label: '事業の割合', control: h('div', { class: 'affix' }, u.pctIn, h('span', { class: 'affix-text' }, '%'), u.ownDraw), labelFor: u.pctIn, after: u.pctNote.el, hint: phrase('仕事に使った分の割合。残りは事業主貸に入ります。'), className: 'span-2' });

  u.grid = h('div', { class: 'entry-grid' }, u.dateF.el, u.amountF.el, u.accountF.el, u.toWalletF.el, u.walletF.el, u.taxF.el, u.detailCell, u.lockCell, u.partnerF.el, u.descF.el, u.pctF.el);
  return u;
}
