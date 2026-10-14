// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の表の1行（SC-03）。欄は 候補・日付・摘要・金額・科目・税区分・取引先・メモ・割合・操作。
// 欄を離れるときとEnterで PATCH /drafts/{id} に保存する。同じ行の保存は順に送る（追い越して古い値が残らないように）。
// 科目を変えたときの税区分と割合は account-defaults.js の決まり（変える前の科目の既定値と同じなら更新）。
// 免税のときは税区分の列を隠し、既定の税区分を入れない（確定のときは空で送る。2026-10-14 20:10 の決定）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatSigned } from '../format.js';
import { normalizeWidth } from '../input-parse.js';
import { nextDefaults, taxOptions, isTransitional } from '../account-defaults.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { selectInput, textInput, iconButton } from '../components/fields.js';
import { accountItems, partnerItems } from './entry-options.js';

// 表の狭い列に収まる、税区分の短い名前（選んである名前の全体は、欄の title と行の下の案内で示す）。
const TAX_SHORT = { '': '対象外', S10: '課税売上10%', S8: '課税売上8%軽減', P10: '課税仕入10%', P8: '課税仕入8%軽減', P10X: '経過措置10%', P8X: '経過措置8%軽減', NT: '非課税' };

/** 列の順。上下の矢印は、この名前の同じ列の隣の行へ移る。 */
export const COLUMNS = ['cand', 'date', 'desc', 'amount', 'account', 'tax', 'partner', 'memo', 'pct', 'ops'];

/** 「10/01」。年が違うときだけ「2025/12/30」。 */
export function shortDate(iso, thisYear) {
  return iso.slice(0, 4) === thisYear ? `${iso.slice(5, 7)}/${iso.slice(8, 10)}` : iso.replace(/-/g, '/');
}

/**
 * @param {{ draft: object, accounts: any[], partners: any[], exempt: boolean, readOnly: boolean, thisYear: string,
 *           colId: (col: string) => string, api: any, candidates: ReturnType<import('../candidates.js').createCandidates>,
 *           onCandidate: () => void, onNext: (row: object) => void, onIgnore: (row: object) => void,
 *           onStale: (err: Error) => void, onSaveState: (kind: 'saving'|'saved'|'failed') => void }} o
 */
export function createInboxRow(o) {
  let draft = { ...o.draft };
  const byCode = new Map(o.accounts.map((a) => [a.code, a]));
  const kind = draft.direction === 'in' ? 'income' : 'expense';
  const descId = `draft-${draft.id}-desc`;
  const msgId = `draft-${draft.id}-msg`;
  let chain = Promise.resolve();
  const errors = {};

  const labelled = (el, col) => {
    el.setAttribute('aria-labelledby', `${o.colId(col)} ${descId}`);
    el.setAttribute('aria-describedby', msgId);
    if (o.readOnly) el.disabled = true;
    return el;
  };

  const check = labelled(h('input', { type: 'checkbox', class: 'cand-check' }), 'cand');
  const date = createDateInput({ value: draft.date, arrowKeys: false, display: (iso) => shortDate(iso, o.thisYear), placeholder: '', inputClass: 'input--date', onError: (m) => setError('date', m), onChange: (v) => v && v !== draft.date && save({ date: v }) });
  labelled(date.input, 'date');
  const account = createCombobox({
    items: accountItems(o.accounts, kind),
    value: draft.account,
    arrowOpens: false,
    listLabel: '科目の候補',
    placeholder: '科目',
    onSelect: (item, info) => {
      // 打っている途中（選択が外れただけ）は保存しない。決めるのは、選んだときと、欄を離れたとき。
      if (info.via === 'input') return undefined;
      chooseAccount(item ? item.value : null);
      if (info.via === 'enter' && item) {
        markAndNext();
        return 'stop';
      }
      return undefined;
    },
  });
  labelled(account.input, 'account');
  const taxChoices = taxOptions(kind);
  const tax = labelled(selectInput({ value: draft.tax_code || '' }, taxChoices.map((x) => ({ value: x.value, label: TAX_SHORT[x.value] || x.label }))), 'tax');
  const taxTitle = () => {
    tax.title = (taxChoices.find((x) => x.value === tax.value) || { label: '' }).label;
  };
  const partner = createCombobox({ items: partnerItems(o.partners), allowFree: true, arrowOpens: false, value: draft.partner || '', listLabel: '取引先の候補', placeholder: '取引先' });
  labelled(partner.input, 'partner');
  const memo = labelled(textInput({ value: draft.memo || '', placeholder: 'メモ', maxlength: 200 }), 'memo');
  const pct = draft.direction === 'out' ? labelled(textInput({ value: String(draft.business_pct ?? 100), inputmode: 'numeric', class: 'input--num pct-cell', maxlength: 3 }), 'pct') : null;
  const ignoreBtn = iconButton({ iconName: 'ban', label: `取り込まない（x）: ${draft.description}`, className: 'ignore-btn', onClick: () => o.onIgnore(api) });
  if (o.readOnly) ignoreBtn.disabled = true;

  const msg = h('div', { class: 'row-msg', id: msgId, 'aria-live': 'polite' });
  const cell = (col, label, ...children) => h('td', { class: `c-${col}`, dataset: { col }, 'data-label': label }, children);
  const tr = h(
    'tr',
    { class: 'inbox-row', dataset: { id: draft.id, dir: draft.direction } },
    cell('cand', '確定候補', h('label', { class: 'cand' }, check, h('span', { class: 'cand-text' }, '候補'))),
    cell('date', '日付', date.input),
    cell('desc', '', h('span', { class: 'desc-text', id: descId }, draft.description || '（摘要なし）'), h('span', { class: 'sub' }, draft.wallet_name)),
    cell('amount', '金額', h('span', { class: ['amount', `amount--${draft.direction}`] }, formatSigned(draft.direction === 'in' ? draft.amount : -draft.amount)), h('span', { class: 'sub dir-text' }, draft.direction === 'in' ? '入金' : '出金')),
    cell('account', '科目', account.el),
    cell('tax', '税区分', tax),
    cell('partner', '取引先', partner.el),
    cell('memo', 'メモ', memo),
    cell('pct', '割合', pct ? h('div', { class: 'affix' }, pct, h('span', { class: 'affix-text' }, '%')) : h('span', { class: 'muted' }, '—')),
    cell('ops', '', ignoreBtn),
  );
  const msgTr = h('tr', { class: 'inbox-msg', hidden: true }, h('td', { colspan: COLUMNS.length }, msg));

  function paintMessages() {
    taxTitle();
    const parts = [];
    for (const [field, text] of Object.entries(errors)) if (text) parts.push(h('p', { class: 'field-error', dataset: { field } }, icon('alert', { size: 16 }), h('span', null, text)));
    if (!o.exempt && isTransitional(draft.tax_code)) parts.push(h('p', { class: 'field-hint row-note' }, icon('info', { size: 16 }), h('span', null, '課税仕入れの日にしてください（商品の引渡し・役務の提供が完了した日）。経過措置の割合は、この行の日付で決まります。')));
    msg.replaceChildren(...parts);
    msgTr.hidden = !parts.length;
    tr.classList.toggle('has-msg', parts.length > 0);
  }

  function setError(field, text) {
    errors[field] = text || '';
    paintMessages();
  }

  function locked() {
    const a = byCode.get(draft.account);
    return Boolean(a && a.wallet);
  }

  // 相手が口座の行（カードの引き落としなど）は、税区分と割合を使わない（AC-02.8 と同じ決まり）。
  function paintLocks() {
    const lock = locked() || o.readOnly;
    tax.disabled = lock;
    if (pct) pct.disabled = lock;
  }

  function save(patch) {
    if (o.readOnly) return chain;
    o.onSaveState('saving');
    // 送る前に手元の値を変えておく。応えを待つ間に次の欄を変えても、既定値の決まりが新しい値で判断できるように。
    draft = { ...draft, ...patch };
    chain = chain
      .then(() => o.api.patch(`/drafts/${draft.id}`, patch))
      .then((res) => {
        if (res && typeof res === 'object') draft = { ...draft, ...res };
        else draft = { ...draft, ...patch };
        Object.keys(patch).forEach((k) => setError(k === 'tax_code' ? 'tax' : k, ''));
        o.onSaveState('saved');
      })
      .catch((err) => {
        o.onSaveState('failed');
        if (err.code === 'already_confirmed' || err.status === 404 || err.isStale) o.onStale(err);
        else setError(err.field === 'tax_code' ? 'tax' : err.field || 'row', err.message);
      });
    return chain;
  }

  function chooseAccount(code) {
    if ((code || null) === (draft.account || null)) return;
    const prev = byCode.get(draft.account) || null;
    const next = byCode.get(code) || null;
    const d = nextDefaults({ prev, next, tax: draft.tax_code, pct: draft.business_pct ?? 100 });
    const valid = taxOptions(kind).map((x) => x.value);
    // 免税のときは、隠れた税区分に科目の既定値を入れない（空で送る）。
    const taxCode = o.exempt ? null : d.tax && valid.includes(d.tax) ? d.tax : null;
    const patch = { account: code || null };
    if (taxCode !== (draft.tax_code || null)) patch.tax_code = taxCode;
    const nextPct = draft.direction === 'out' ? d.pct : 100;
    if (nextPct !== (draft.business_pct ?? 100)) patch.business_pct = nextPct;
    draft = { ...draft, ...patch };
    tax.value = draft.tax_code || '';
    if (pct) pct.value = String(draft.business_pct ?? 100);
    setError('account', '');
    paintLocks();
    paintMessages();
    if (!code && o.candidates.has(draft.id)) setCandidate(false);
    save(patch);
  }

  function setCandidate(on) {
    if (on && !draft.account) {
      setError('account', '科目を選んでください。科目が空の行は、確定候補にできません。');
      check.checked = false;
      return false;
    }
    if (on) {
      o.candidates.add(draft);
      // 免税のときは、隠れた税区分を空にしてから確定する（前に課税の設定で入れた値が残っていても送らない）。
      if (o.exempt && draft.tax_code) {
        draft = { ...draft, tax_code: null };
        save({ tax_code: null });
      }
    } else o.candidates.remove(draft.id);
    check.checked = on;
    tr.classList.toggle('is-candidate', on);
    o.onCandidate();
    return true;
  }

  /** 割合の欄を読む。だめなら理由を出して偽。 */
  function readPct() {
    if (!pct || pct.disabled) return true;
    const n = Number(normalizeWidth(pct.value).trim());
    if (pct.value.trim() === '' || !Number.isInteger(n) || n < 0 || n > 100) {
      setError('business_pct', '事業の割合は、0から100の整数で入れてください。');
      return false;
    }
    if (n !== (draft.business_pct ?? 100)) save({ business_pct: n });
    else setError('business_pct', '');
    return true;
  }

  /** いまの欄の入力を保存する（Enterと、確定の前に呼ぶ）。読めない値なら偽。 */
  function commit(col) {
    if (col === 'account') {
      // 欄の文字と、選んである科目を合わせる。打った文字に合う科目が無ければ、前の科目のまま確定候補にしない。
      const v = account.value;
      if ((v || null) !== (draft.account || null)) chooseAccount(v);
      if (!v && account.input.value.trim()) {
        setError('account', '打った文字に合う科目がありません。候補から選んでください。');
        return false;
      }
      return true;
    }
    if (col === 'date') return date.commit();
    if (col === 'partner' && partner.value !== (draft.partner || '')) save({ partner: partner.value });
    if (col === 'memo' && memo.value !== (draft.memo || '')) save({ memo: memo.value });
    if (col === 'pct') return readPct();
    return true;
  }

  /** 行を確定候補にして、次の行へ（Enter）。科目が空なら理由を出して動かない（AC-05.1、AC-05.3）。 */
  function markAndNext(col = 'account') {
    if (!commit(col)) return;
    if (!draft.account) {
      setError('account', '科目を選んでください。科目が空の行は、確定候補にできません。');
      return;
    }
    setCandidate(true);
    o.onNext(api);
  }

  check.addEventListener('change', () => setCandidate(check.checked));
  tax.addEventListener('change', () => {
    draft = { ...draft, tax_code: tax.value || null };
    paintMessages();
    save({ tax_code: draft.tax_code });
  });
  // 候補欄の部品の blur（自動の選択）の後に動くよう、部品を作った後に登録する。
  account.input.addEventListener('blur', () => commit('account'));
  partner.input.addEventListener('blur', () => commit('partner'));
  memo.addEventListener('blur', () => commit('memo'));
  if (pct) pct.addEventListener('blur', () => readPct());

  const api = {
    tr,
    msgTr,
    get id() {
      return draft.id;
    },
    get draft() {
      return draft;
    },
    /** 列の欄へフォーカスを移す（無い列なら候補の欄）。 */
    focus(col) {
      const el = { cand: check, date: date.input, account: account.input, tax, partner: partner.input, memo, pct }[col];
      const target = el && !el.disabled && !el.closest('[hidden]') && el.offsetParent !== null ? el : check;
      target.focus();
      if (target.select && target !== check) target.select();
    },
    isOpen: () => account.isOpen || partner.isOpen,
    setCandidate,
    markAndNext,
    commit,
    /** 送り終えていない保存を待つ（確定の前）。 */
    flush: () => chain,
    showError: (text) => setError('row', text),
    destroy() {
      account.destroy();
      partner.destroy();
    },
  };
  check.checked = o.candidates.has(draft.id);
  tr.classList.toggle('is-candidate', check.checked);
  paintLocks();
  paintMessages();
  return api;
}
