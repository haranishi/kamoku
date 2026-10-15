// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の表の1行（SC-03）。欄は 確定する・日付・摘要・金額・科目・税区分・取引先・メモ・割合・操作（作るのは inbox-row-fields.js）。
// 欄を離れるときとEnterで PATCH /drafts/{id} に保存する。同じ行の保存は順に送る（追い越して古い値が残らないように）。
// 科目を変えたときの税区分は inbox-rules.js（手で選んでいなければ新しい科目の既定へ）、割合は account-defaults.js の決まり。
// 免税のときは税区分の列を隠し、既定の税区分を入れない（確定のときは空で送る。2026-10-14 20:10 の決定）。
// カードの並び（入れ物が1080px未満）では、要約（日付・摘要・金額・口座）と科目の欄だけを出し、残りは「くわしく」で開く
// （screens-inbox-cards.css）。そのときの欄の並び（Tabの順）は、見た目の順にそろえる（inbox-row-parts.js の orderCells）。

import { h } from '../dom.js';
import { normalizeWidth } from '../input-parse.js';
import { nextDefaults } from '../account-defaults.js';
import { shortDate, paintTaxSelect, rowMessages, TAX_UNSET, moreToggle, rowElement, FOLDED, fieldOf } from './inbox-row-parts.js';
import { createRowFields } from './inbox-row-fields.js';
import { taxAfterAccount, touchedOnLoad, rowLabel } from './inbox-rules.js';

export { shortDate };

/** 列の順。上下の矢印は、この名前の同じ列の隣の行へ移る（税区分の欄では、値の切り替えに使う）。 */
export const COLUMNS = ['cand', 'date', 'desc', 'amount', 'account', 'tax', 'partner', 'memo', 'pct', 'ops'];

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
  const ids = (part) => `draft-${draft.id}-${part}`;
  const msgId = ids('msg');
  let chain = Promise.resolve();
  const errors = {};
  const nameEl = h('span', { class: 'visually-hidden', id: ids('name') });

  // 欄（inbox-row-fields.js）。呼び返しは、どれも作り終えた後の操作で呼ばれる。
  const { check, sumDate, date, account, tax, partner, memo, pct, ignore } = createRowFields({
    draft,
    kind,
    accounts: o.accounts,
    partners: o.partners,
    readOnly: o.readOnly,
    thisYear: o.thisYear,
    colId: o.colId,
    nameId: nameEl.id,
    msgId,
    onDate: (v) => v && v !== draft.date && (sumDate.replaceChildren(shortDate(v, o.thisYear)), save({ date: v }), paintName()),
    onDateError: (m) => setError('date', m),
    onAccountSelect: (item, info) => {
      // 打っている途中（選択が外れただけ）は保存しない。決めるのは、選んだときと、欄を離れたとき。
      if (info.via === 'input') return undefined;
      chooseAccount(item ? item.value : null);
      if (info.via === 'enter' && item) {
        markAndNext();
        return 'stop';
      }
      return undefined;
    },
    onIgnore: () => o.onIgnore(api),
  });
  // 税区分を利用者が手で選んだか。選んでいなければ、科目を変えるたびに科目の既定へ入れ直す（inbox-rules.js）。
  let taxTouched = touchedOnLoad({ account: byCode.get(draft.account) || null, kind, exempt: o.exempt, tax: draft.tax_code });

  const msg = h('div', { class: 'row-msg', id: msgId, 'aria-live': 'polite' });
  const more = moreToggle({ controls: ['date', 'tax', 'partner', 'memo', 'pct'].map((c) => ids(`cell-${c}`)), onToggle: (open) => setOpen(open) });
  const tr = rowElement({ draft, cellId: (col) => ids(`cell-${col}`), descId: ids('desc'), name: nameEl, check, date: date.input, sumDate, account: account.el, more: more.el, tax, partner: partner.el, memo, pct, ignore: ignore.el });
  const msgTr = h('tr', { class: 'inbox-msg', hidden: true }, h('td', { colspan: COLUMNS.length }, msg));

  /** 行の名前（日付・摘要・金額）を、読み上げだけの文字と「無視」のボタンに入れる。 */
  function paintName() {
    const text = rowLabel(draft, o.thisYear);
    nameEl.textContent = text;
    ignore.name(text);
    more.name(text);
  }

  /** 狭い幅の「くわしく」を開く・閉じる（広い幅では見た目が変わらない）。 */
  function setOpen(open) {
    tr.classList.toggle('is-folded', !open);
    more.set(open);
  }

  /** 科目が空の行の印（is-unset）と、税区分の見せ方（手で選んでいない空の税区分は、仮の値で薄く出す）。 */
  function paintTax() {
    const none = !draft.account;
    tr.classList.toggle('is-unset', none);
    paintTaxSelect(tax, { code: draft.tax_code, unset: !taxTouched && !draft.tax_code && !locked(), hasAccount: !none, account: byCode.get(draft.account) || null, kind });
  }

  function paintMessages() {
    const parts = rowMessages({ errors, account: byCode.get(draft.account) || null, draft, kind, exempt: o.exempt });
    msg.replaceChildren(...parts);
    msgTr.hidden = !parts.length;
    tr.classList.toggle('has-msg', parts.length > 0);
  }

  function setError(field, text) {
    errors[field] = text || '';
    if (text && FOLDED.includes(field)) setOpen(true);
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
        else setError(fieldOf(err.field), err.message);
      });
    return chain;
  }

  function chooseAccount(code) {
    if ((code || null) === (draft.account || null)) return;
    const prev = byCode.get(draft.account) || null;
    const next = byCode.get(code) || null;
    // 口座（振替）にすると税区分を空にする。手で選んだ値も消えるので、そのあと別の科目にしたら、その科目の既定に戻す。
    if (next && next.wallet) taxTouched = false;
    const taxCode = taxAfterAccount({ next, kind, exempt: o.exempt, tax: draft.tax_code, touched: taxTouched });
    const patch = { account: code || null };
    if (taxCode !== (draft.tax_code || null)) patch.tax_code = taxCode;
    const nextPct = draft.direction === 'out' ? nextDefaults({ prev, next, tax: draft.tax_code, pct: draft.business_pct ?? 100 }).pct : 100;
    if (nextPct !== (draft.business_pct ?? 100)) patch.business_pct = nextPct;
    draft = { ...draft, ...patch };
    if (pct) pct.value = String(draft.business_pct ?? 100);
    errors.account = '';
    paintTax();
    paintLocks();
    paintMessages();
    if (!code && o.candidates.has(draft.id)) setCandidate(false);
    else o.onCandidate();
    save(patch);
  }

  // 無視を送っている途中（inbox-ignore.js）。確定する行にしない（確定と無視が行き違わないように）。
  let leaving = false;

  function setCandidate(on) {
    if (on && leaving) {
      check.checked = false;
      return false;
    }
    if (on && !draft.account) {
      setError('account', '科目を選んでください。科目が空の行は、確定できません。');
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
      // 欄の文字と、選んである科目を合わせる。打った文字に合う科目が無ければ、前の科目のまま確定する行にしない。
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

  /** 行を確定する行にして、次の行へ（Enter）。科目が空なら理由を出して動かない（AC-05.1、AC-05.3）。 */
  function markAndNext(col = 'account') {
    if (!commit(col)) return;
    if (!draft.account) {
      setError('account', '科目を選んでください。科目が空の行は、確定できません。');
      return;
    }
    setCandidate(true);
    o.onNext(api);
  }

  check.addEventListener('change', () => setCandidate(check.checked));
  tax.addEventListener('change', () => {
    taxTouched = true;
    draft = { ...draft, tax_code: tax.value === TAX_UNSET ? null : tax.value || null };
    paintTax();
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
    /** 科目が入っていて、確定する行にできるか（「科目のある行をすべて選ぶ」が使う）。 */
    get ready() {
      return Boolean(draft.account) && !leaving;
    },
    /** 無視を送っている間の印（薄く出す）。付けるときは、確定する行から外す。 */
    leave(on) {
      leaving = on;
      tr.classList.toggle('is-leaving', on);
      if (on && o.candidates.has(draft.id)) setCandidate(false);
      else o.onCandidate();
    },
    /**
     * 列の欄へフォーカスを移す（無い列なら、行の印）。nearest が真なら、行が画面の端に半分隠れないよう、
     * 近い端へ寄せて見せる（上下の固定の帯の分は、ページの scroll-padding が除く）。
     */
    focus(col, { nearest = false } = {}) {
      const el = { cand: check, date: date.input, account: account.input, tax, partner: partner.input, memo, pct }[col];
      const target = el && !el.disabled && !el.closest('[hidden]') && el.offsetParent !== null ? el : check;
      target.focus({ preventScroll: nearest });
      if (nearest) tr.scrollIntoView({ block: 'nearest' });
      if (target.select && target !== check) target.select();
    },
    isOpen: () => account.isOpen || partner.isOpen,
    setCandidate,
    markAndNext,
    commit,
    /** 送り終えていない保存を待つ（確定の前）。 */
    flush: () => chain,
    /** 確定などで返った誤りを出す。field（サーバーの欄の名前）が畳んだ欄なら、「くわしく」を開く。 */
    showError: (text, field) => setError(fieldOf(field), text),
    destroy() {
      account.destroy();
      partner.destroy();
    },
  };
  check.checked = o.candidates.has(draft.id);
  tr.classList.toggle('is-candidate', check.checked);
  paintName();
  paintTax();
  paintLocks();
  paintMessages();
  return api;
}
