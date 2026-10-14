// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 登録簿の追加と編集のダイアログ（口座・勘定科目・取引先）。初期設定の口座の段階でも、口座のダイアログを使う。
// サーバーの誤りに field があれば、その欄のすぐ下に出す（nfr.md）。無ければダイアログの下に出す。

import { h } from '../dom.js';
import { shouldIgnoreKey } from '../ime.js';
import { normalizeWidth } from '../input-parse.js';
import { normalizeInvoice } from '../invoice.js';
import { TAX_LABELS } from '../account-defaults.js';
import { field, textInput, selectInput } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { openModal } from '../components/dialog.js';

export const WALLET_TYPES = [
  { value: 'cash', label: '現金', parent: 'cash' },
  { value: 'bank', label: '預金', parent: 'dep_other' },
  { value: 'card', label: 'クレジットカード', parent: 'accrued' },
  { value: 'ewallet', label: '電子マネー', parent: 'cash' },
];
export const KIND_LABEL = { asset: '資産', liability: '負債', equity: '資本', revenue: '収益', expense: '費用' };
/** 名前を変えられない科目（帳簿の決まりで使う。2026-10-14 20:10 の決定）。 */
export const NAME_LOCKED = new Set(['owner_draw', 'owner_inv', 'capital']);
/** 無効にできない科目（帳簿に必ず要る。kamoku-books の REQUIRED_ACCOUNTS）。 */
export const REQUIRED = new Set(['cash', 'dep_other', 'accrued', 'owner_draw', 'owner_inv', 'capital', 'sales']);
const TAX_CHOICES = [{ value: '', label: '対象外（なし）' }, ...Object.entries(TAX_LABELS).map(([value, label]) => ({ value, label }))];

function check(label, checked, hint) {
  const box = h('input', { type: 'checkbox', checked });
  return { box, el: h('div', { class: 'field' }, h('label', { class: 'check' }, box, h('span', null, label)), hint ? h('p', { class: 'field-hint' }, hint) : null) };
}

/**
 * 入力のダイアログを開き、保存できたら値を返す（やめたら null）。
 * save(ctrl) は、保存して応えを返す。欄の誤りは fields[name].setError で出す。
 */
function formDialog({ title, body, submitLabel, fields, save }) {
  return new Promise((resolve) => {
    let result = null;
    // 文字の欄で Enter（変換中を除く）を押したら、保存のボタンを押したのと同じにする。
    body.addEventListener('keydown', (e) => {
      if (shouldIgnoreKey(e) || e.key !== 'Enter' || e.target.tagName !== 'INPUT' || ['checkbox', 'radio'].includes(e.target.type)) return;
      e.preventDefault();
      const primary = body.closest('dialog') && body.closest('dialog').querySelector('.dialog-foot .btn--primary');
      if (primary && !primary.disabled) primary.click();
    });
    openModal({
      title,
      body,
      className: 'form-dialog',
      actions: [
        { label: 'やめる', onClick: (c) => c.close('cancel') },
        {
          label: submitLabel,
          variant: 'primary',
          onClick: async (c) => {
            c.setError('');
            Object.values(fields).forEach((f) => f.setError(''));
            c.setBusy(true);
            try {
              const res = await save(c);
              c.setBusy(false);
              if (res === undefined) return;
              result = res;
              c.close('ok');
            } catch (err) {
              c.setBusy(false);
              const f = err.field && fields[err.field];
              if (f) {
                f.setError(err.message);
                const input = f.el.querySelector('input, select, textarea');
                if (input) input.focus();
              } else c.setError(err.message);
            }
          },
        },
      ],
      onClose: () => resolve(result),
    });
  });
}

/** 割合の欄を読む。0〜100の整数なら数、だめなら null。 */
function readPct(input) {
  const n = Number(normalizeWidth(input.value).trim());
  return input.value.trim() !== '' && Number.isInteger(n) && n >= 0 && n <= 100 ? n : null;
}

/**
 * 口座を足す・直す。型は「現金」「預金」「クレジットカード」「電子マネー」から選ぶ（AC-01.2）。型はあとから変えられない。
 * @param {{ api: any, wallet?: object, defaultType?: string }} o
 * @returns {Promise<object|null>} 保存した科目（口座）
 */
export function walletDialog({ api, wallet, defaultType = 'bank' }) {
  const editing = Boolean(wallet);
  const type = editing ? null : createSegmented({ legend: '口座の型', options: WALLET_TYPES.map((t) => ({ value: t.value, label: t.label })), value: defaultType, className: 'seg--grid' });
  const nameIn = textInput({ value: wallet ? wallet.name : '', maxlength: 60, placeholder: '例: サンプル銀行 普通預金' });
  const readingIn = textInput({ value: wallet ? wallet.reading : '', maxlength: 60, placeholder: '例: さんぷるぎんこう' });
  const instIn = editing ? null : textInput({ maxlength: 60, placeholder: '例: サンプル銀行' });
  const fields = {
    name: field({ label: '名前', control: nameIn, required: true, hint: '画面に出る名前です。自由に付けられます。' }),
    reading: field({ label: '読み（ひらがな）', control: readingIn, hint: '科目や口座の欄で、読みでも探せるようになります。' }),
  };
  if (instIn) fields.institution = field({ label: '金融機関の名前（任意）', control: instIn });
  const enabled = editing ? check('この口座を使う', wallet.enabled, '使わなくなった口座は外せます。過去の仕訳は変わりません。口座は消せません。') : null;
  const body = h('div', { class: 'stack' }, type ? type.el : h('p', { class: 'muted small' }, `型: ${(WALLET_TYPES.find((t) => wallet.wallet && t.value === wallet.wallet.type) || { label: '' }).label}（型はあとから変えられません）`), fields.name.el, fields.reading.el, fields.institution ? fields.institution.el : null, enabled ? enabled.el : null);
  return formDialog({
    title: editing ? `「${wallet.name}」を直す` : '口座を足す',
    body,
    submitLabel: editing ? '保存する' : '足す',
    fields,
    save: async () => {
      const name = nameIn.value.trim();
      if (!name) {
        fields.name.setError('名前を入れてください。');
        nameIn.focus();
        return undefined;
      }
      if (editing) return api.patch(`/accounts/${wallet.code}`, { name, reading: readingIn.value.trim(), enabled: enabled.box.checked });
      const t = WALLET_TYPES.find((x) => x.value === type.value);
      return api.post('/accounts', { name, reading: readingIn.value.trim(), kind: t.value === 'card' ? 'liability' : 'asset', parent: t.parent, wallet: { type: t.value, institution: instIn.value.trim() }, default_tax: null, default_pct: 100 });
    },
  });
}

/**
 * 勘定科目を足す・直す。分類は変えられない。owner_draw owner_inv capital は名前を変えられない。
 * @param {{ api: any, account?: object, accounts: any[] }} o
 */
export function accountDialog({ api, account, accounts }) {
  const editing = Boolean(account);
  const isWallet = Boolean(account && account.wallet);
  const kindSel = editing ? null : selectInput({ value: 'expense' }, Object.entries(KIND_LABEL).map(([value, label]) => ({ value, label })));
  const parentSel = editing ? null : selectInput({}, [{ value: '', label: 'なし（いちばん上）' }]);
  const fillParents = () => {
    parentSel.replaceChildren(h('option', { value: '' }, 'なし（いちばん上）'), ...accounts.filter((a) => a.kind === kindSel.value && !a.wallet && a.enabled).map((a) => h('option', { value: a.code }, a.name)));
  };
  const nameIn = textInput({ value: account ? account.name : '', maxlength: 60 });
  const locked = editing && NAME_LOCKED.has(account.code);
  nameIn.disabled = locked;
  const readingIn = textInput({ value: account ? account.reading : '', maxlength: 60 });
  const taxSel = selectInput({ value: account ? account.default_tax || '' : '' }, TAX_CHOICES);
  const pctIn = textInput({ value: String(account ? account.default_pct ?? 100 : 100), inputmode: 'numeric', class: 'input--num pct-input', maxlength: 3 });
  const fields = {
    name: field({ label: '名前', control: nameIn, required: true, hint: locked ? 'この科目は帳簿の決まりで使うので、名前を変えられません。' : '標準の科目も、名前を変えられます。過去の仕訳は、科目のコードで残っています。' }),
    reading: field({ label: '読み（ひらがな）', control: readingIn }),
    default_tax: field({ label: '既定の税区分', control: taxSel, hint: '科目を選んだときに入る初期値です。' }),
    default_pct: field({ label: '既定の事業割合', control: h('div', { class: 'affix' }, pctIn, h('span', { class: 'affix-text' }, '%')), labelFor: pctIn, hint: '仕事と私用が混ざる科目は、仕事の分の割合を入れておくと、選んだときに入ります。' }),
  };
  if (kindSel) {
    fields.kind = field({ label: '分類', control: kindSel, required: true, hint: '分類はあとから変えられません。' });
    fields.parent = field({ label: '親の科目（任意）', control: parentSel, hint: '帳票で、親の科目の下にまとめて出します。' });
    kindSel.addEventListener('change', fillParents);
    fillParents();
  }
  const enabled = editing ? check('この科目を使う', account.enabled, REQUIRED.has(account.code) ? '帳簿に必ず要る科目なので、外せません。' : '使わない科目は外せます。過去の仕訳は変わりません。') : null;
  if (enabled && REQUIRED.has(account.code)) enabled.box.disabled = true;
  const pl = !account || account.kind === 'revenue' || account.kind === 'expense';
  const body = h(
    'div',
    { class: 'stack' },
    editing ? h('p', { class: 'muted small' }, `分類: ${KIND_LABEL[account.kind]}　コード: ${account.code}${isWallet ? '（口座）' : ''}`) : null,
    fields.kind ? fields.kind.el : null,
    fields.name.el,
    fields.reading.el,
    fields.parent ? fields.parent.el : null,
    !isWallet && pl ? fields.default_tax.el : null,
    !isWallet && (!account || account.kind === 'expense') ? fields.default_pct.el : null,
    enabled ? enabled.el : null,
  );
  return formDialog({
    title: editing ? `「${account.name}」を直す` : '科目を足す',
    body,
    submitLabel: editing ? '保存する' : '足す',
    fields,
    save: async () => {
      const name = nameIn.value.trim();
      if (!name) {
        fields.name.setError('名前を入れてください。');
        nameIn.focus();
        return undefined;
      }
      const pct = readPct(pctIn);
      if (pct === null) {
        fields.default_pct.setError('0から100の整数で入れてください。');
        pctIn.focus();
        return undefined;
      }
      if (editing) {
        const patch = { reading: readingIn.value.trim(), enabled: enabled.box.checked };
        if (!locked) patch.name = name;
        if (!isWallet) Object.assign(patch, { default_tax: taxSel.value || null, default_pct: pct });
        return api.patch(`/accounts/${account.code}`, patch);
      }
      return api.post('/accounts', { name, reading: readingIn.value.trim(), kind: kindSel.value, parent: parentSel.value || null, wallet: null, default_tax: taxSel.value || null, default_pct: pct });
    },
  });
}

/**
 * 取引先を足す・直す。登録番号は形式（T+13桁）だけを確かめ、誤りは欄の下に出す（SC-09）。
 * @param {{ api: any, partner?: object }} o
 */
export function partnerDialog({ api, partner }) {
  const editing = Boolean(partner);
  const nameIn = textInput({ value: partner ? partner.name : '', maxlength: 80, placeholder: '例: サンプル商事' });
  const readingIn = textInput({ value: partner ? partner.reading : '', maxlength: 80, placeholder: '例: さんぷるしょうじ' });
  const invIn = textInput({ value: partner ? partner.invoice_number || '' : '', maxlength: 20, placeholder: '例: T1234567890123', inputmode: 'latin', autocapitalize: 'characters' });
  const noteIn = textInput({ value: partner ? partner.note : '', maxlength: 200 });
  const fields = {
    name: field({ label: '名前', control: nameIn, required: true }),
    reading: field({ label: '読み（ひらがな）', control: readingIn, hint: '取引先の欄で、読みでも探せるようになります。' }),
    invoice_number: field({ label: '登録番号（任意）', control: invIn, hint: '適格請求書発行事業者の番号です。形式（T と13桁の数字）だけを確かめます。' }),
    note: field({ label: 'メモ', control: noteIn }),
  };
  const enabled = editing ? check('この取引先を使う', partner.enabled !== false, '使わない取引先は、候補に出なくなります。') : null;
  const checkInvoice = () => {
    const r = normalizeInvoice(invIn.value);
    fields.invoice_number.setError(r.ok ? '' : r.error);
    return r;
  };
  invIn.addEventListener('blur', checkInvoice);
  return formDialog({
    title: editing ? `「${partner.name}」を直す` : '取引先を足す',
    body: h('div', { class: 'stack' }, fields.name.el, fields.reading.el, fields.invoice_number.el, fields.note.el, enabled ? enabled.el : null),
    submitLabel: editing ? '保存する' : '足す',
    fields,
    save: async () => {
      const name = nameIn.value.trim();
      if (!name) {
        fields.name.setError('名前を入れてください。');
        nameIn.focus();
        return undefined;
      }
      const inv = checkInvoice();
      if (!inv.ok) {
        invIn.focus();
        return undefined;
      }
      const body = { name, reading: readingIn.value.trim(), invoice_number: inv.value, note: noteIn.value.trim() };
      if (editing) return api.patch(`/partners/${partner.id}`, { ...body, enabled: enabled.box.checked });
      return api.post('/partners', body);
    },
  });
}
