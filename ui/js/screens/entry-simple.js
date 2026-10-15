// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の入力欄（単純な形: 支出・収入・振替）。並びは keyboard.md のとおり。
// Enterで次の欄へ進み、最後の欄のEnterと Ctrl/Cmd+Enter で保存の合図（onSubmit）を出す。変換中のEnterは何もしない。

import { h, addDescribedBy, removeDescribedBy } from '../dom.js';
import { shouldIgnoreKey } from '../ime.js';
import { todayIso } from '../input-parse.js';
import { formatYen, formatDate } from '../format.js';
import { nextDefaults, taxOptions, isTransitional } from '../account-defaults.js';
import { accountItems, defaultWallet } from './entry-options.js';
import { createPreviewRunner } from './entry-run-preview.js';
import { buildSimpleFields, WALLET_LABEL } from './entry-simple-fields.js';
import { checkSimple, fieldNames } from './entry-simple-check.js';
import { parsePct, PCT_ERROR, pctSourceNote, taxSourceNote } from './entry-hints.js';

/**
 * @param {{ accounts: any[], partners: any[], settings: any, initial?: any, preview: any, api: any, labelledBy?: string,
 *           onSubmit: (opts: { cont: boolean }) => void, onCancel: () => void, onDirty?: () => void }} opts
 *   labelledBy: フォームの名前にする見出しの id（読み上げで「取引を登録」のフォームと伝わる）
 */
export function createSimpleForm(opts) {
  const byCode = new Map(opts.accounts.map((a) => [a.code, a]));
  const exempt = opts.settings && opts.settings.tax_mode === 'exempt';
  const taxable = Boolean(opts.settings && opts.settings.tax_mode === 'taxable');
  const init = opts.initial || {};
  const s = {
    kind: init.kind || 'expense',
    date: init.date || todayIso(),
    amount: init.amount ?? null,
    account: init.account ?? null,
    wallet: init.wallet ?? defaultWallet(opts.accounts),
    to_wallet: init.to_wallet ?? null,
    tax_code: init.tax_code ?? null,
    partner: init.partner || '',
    description: init.description || '',
    business_pct: init.business_pct ?? 100,
    memo: init.memo || '',
  };
  let detailOpen = Boolean(init.tax_code) && exempt;
  let baseline = '';
  const changed = () => {
    if (opts.onDirty) opts.onDirty();
    syncNotes();
    runner.schedule();
  };

  const u = buildSimpleFields({
    accounts: opts.accounts,
    partners: opts.partners,
    s,
    on: {
      changed,
      setKind: (k) => setKind(k),
      setAccount: (code) => setAccount(code),
      syncNotes: () => syncNotes(),
      validatePct: () => validatePct(),
      // 日付が決まったら、締めた日以前かを確かめる（部品は値を入れる前に誤りを消すので、その後に出す）。
      dateChanged: (v) => {
        const m = v && opts.dateProblem ? opts.dateProblem(v) : '';
        if (m) u.dateF.setError(m);
      },
      openDetail: () => {
        detailOpen = true;
        // 免税で隠していた間は空にしていたので、欄を出したときに、いまの科目の既定の税区分を入れる（2026-10-14 20:10 の決定）。
        const acc = byCode.get(s.account);
        const valid = taxOptions(s.kind).map((o) => o.value);
        if (!s.tax_code && acc && !acc.wallet && acc.default_tax && valid.includes(acc.default_tax)) {
          s.tax_code = acc.default_tax;
          u.taxSel.value = s.tax_code;
          changed();
        }
        applyKind();
        u.taxSel.focus();
      },
    },
  });
  const { kindSeg, dateIn, dateF, transitionalNote, amountIn, amountF, accountCb, accountF, toWalletCb, toWalletF, walletCb, walletF } = u;
  const { taxSel, taxF, detailBtn, detailCell, lockCell, partnerCb, partnerF, descIn, pctIn, pctF, ownDraw, grid } = u;
  const named = opts.labelledBy ? { 'aria-labelledby': opts.labelledBy } : { 'aria-label': '取引の入力' };
  const form = h('form', { class: 'entry-form', novalidate: true, ...named }, kindSeg.el, grid);

  const runner = createPreviewRunner({
    api: opts.api,
    preview: opts.preview,
    build: () => buildInput(),
    blocked: () => pctProblem(),
    onLines: (lines) => paintOwnDraw(lines),
    onFailedChange: opts.onPreviewState,
    idleText: '日付・金額・科目・口座を入れると、ここに保存される仕訳が出ます。',
  });

  function isLocked() {
    const acc = byCode.get(s.account);
    return s.kind !== 'transfer' && Boolean(acc && acc.wallet);
  }

  let lastLines = [];
  function paintOwnDraw(lines) {
    lastLines = lines || [];
    const own = lastLines.filter((l) => l.account === 'owner_draw' && l.side === 'debit').reduce((a, l) => a + l.amount, 0);
    ownDraw.textContent = s.kind === 'expense' && own ? `→ 事業主貸 ${formatYen(own)}円` : '';
  }

  // 欄の下の一言（経過措置の日付、割合の出どころ、課税で「対象外」のまま、新しい取引先）。値は変えず、気づけるようにするだけ。
  function syncNotes() {
    const expense = s.kind === 'expense' && !isLocked();
    const show = expense && isTransitional(s.tax_code);
    transitionalNote.hidden = !show;
    if (show) addDescribedBy(dateIn.input, transitionalNote.id);
    else removeDescribedBy(dateIn.input, transitionalNote.id);
    const acc = byCode.get(s.account) || null;
    u.pctNote.set(expense ? pctSourceNote(acc) : '');
    u.taxNote.set(taxF.el.hidden ? '' : taxSourceNote({ taxable, kind: s.kind, account: acc, taxCode: s.tax_code }));
    const p = partnerCb.item;
    u.partnerNote.set(p && p.isNew && partnerCb.input.value.trim() === p.label ? `新しい取引先: ${p.label}` : '', { plain: true });
  }

  function applyKind() {
    const k = s.kind;
    const locked = isLocked();
    accountF.el.hidden = k === 'transfer';
    toWalletF.el.hidden = k !== 'transfer';
    walletF.setLabel(WALLET_LABEL[k]);
    const taxVisible = k !== 'transfer' && !locked && (!exempt || detailOpen);
    taxF.el.hidden = !taxVisible;
    detailCell.hidden = !(k !== 'transfer' && !locked && exempt && !detailOpen);
    detailBtn.setAttribute('aria-expanded', detailOpen ? 'true' : 'false');
    lockCell.hidden = !locked;
    pctF.el.hidden = k !== 'expense' || locked;
    syncNotes();
  }

  function setKind(k) {
    if (k === s.kind) return;
    s.kind = k;
    kindSeg.setValue(k);
    accountCb.setItems(accountItems(opts.accounts, k, { forEntry: true, keep: [s.account] }));
    // 種別に合わない税区分は、科目の既定値（無ければ対象外）に戻す。
    const valid = taxOptions(k).map((o) => o.value);
    const current = s.tax_code || '';
    if (!valid.includes(current)) {
      const acc = byCode.get(s.account);
      const def = acc && acc.default_tax && valid.includes(acc.default_tax) ? acc.default_tax : null;
      s.tax_code = exempt && !detailOpen ? null : def;
    }
    taxSel.replaceChildren(...taxOptions(k).map((o) => h('option', { value: o.value }, o.label)));
    taxSel.value = s.tax_code || '';
    if (k !== 'expense') s.business_pct = 100;
    pctIn.value = String(s.business_pct);
    // 割合の欄を正しい値に入れ直したので、前の誤り（150 など）の表示を残さない。
    pctF.setError('');
    applyKind();
    changed();
  }

  function setAccount(code) {
    const prev = byCode.get(s.account) || null;
    const next = byCode.get(code) || null;
    s.account = code;
    const d = nextDefaults({ prev, next, tax: s.tax_code, pct: s.business_pct });
    // 種別に合わない税区分（収入に課税仕入など）は、対象外にする。
    const valid = taxOptions(s.kind).map((o) => o.value);
    // 免税で税区分の欄を隠している間は、科目の既定の税区分を入れない（空で送る。2026-10-14 20:10 の決定）。
    s.tax_code = exempt && !detailOpen ? null : d.tax && valid.includes(d.tax) ? d.tax : null;
    taxSel.value = s.tax_code || '';
    if (s.kind === 'expense') {
      s.business_pct = d.pct;
      pctIn.value = String(d.pct);
      pctF.setError('');
    }
    applyKind();
    changed();
  }

  /** 割合の欄の誤り（支出で、欄が出ているときだけ）。範囲外のうちは、プレビューも保存も止める。 */
  const pctProblem = () => (s.kind !== 'expense' || isLocked() || parsePct(pctIn.value) !== null ? '' : PCT_ERROR);

  function validatePct() {
    const msg = pctProblem();
    pctF.setError(msg);
    return !msg;
  }

  /** 送る形の入力。足りない欄があれば null（プレビューを出さない）。 */
  function buildInput() {
    if (!s.date || !s.amount || !s.wallet) return null;
    const base = { kind: s.kind, date: s.date, amount: s.amount, wallet: s.wallet, partner: s.partner, description: s.description, memo: s.memo };
    if (s.kind === 'transfer') return s.to_wallet ? { ...base, to_wallet: s.to_wallet } : null;
    if (!s.account) return null;
    const locked = isLocked();
    const hiddenTax = exempt && !detailOpen;
    return { ...base, account: s.account, tax_code: locked || hiddenTax ? null : s.tax_code, business_pct: s.kind === 'expense' && !locked ? s.business_pct : 100 };
  }

  // 最後の検査で誤りのあった欄の名前（保存の帯の一言に使う）。
  let problemNames = [];

  /** 保存の前の検査。誤りは欄のすぐ下に出し、最初の誤りへフォーカスを移す。良ければ入力を返す。 */
  function collect() {
    const problems = checkSimple({ u, s, pctProblem });
    problemNames = fieldNames({ u, s }, problems);
    if (problems.length) {
      problems[0].focus();
      return null;
    }
    return buildInput();
  }

  // 入力の指紋。開いたとき（保存して続けたとき）から変わったかを見る（離れる前の確認に使う）。
  // 決まった値に加えて、まだ値になっていない打ちかけの文字（日付・金額・割合・候補の欄）も入れる。
  function fingerprint() {
    const typed = (cb) => (cb.item ? '' : cb.input.value.trim());
    const dateText = dateIn.input.value.trim();
    const dateRaw = s.date && (dateText === s.date || dateText === formatDate(s.date)) ? '' : dateText;
    const amountRaw = amountIn.value === null ? amountIn.input.value.trim() : '';
    return JSON.stringify([s, dateRaw, amountRaw, pctIn.value.trim(), typed(accountCb), typed(walletCb), typed(toWalletCb)]);
  }

  const FIELD_OF = { date: [dateF, () => dateIn.input], amount: [amountF, () => amountIn.input], account: [accountF, () => accountCb.input], wallet: [walletF, () => walletCb.input], to_wallet: [toWalletF, () => toWalletCb.input], tax_code: [taxF, () => taxSel], partner: [partnerF, () => partnerCb.input], business_pct: [pctF, () => pctIn] };

  // 欄の並び（Enterで次へ進む順）。見えている欄だけ。
  function order() {
    return [kindSeg.el, dateIn.input, amountIn.input, accountF.el.hidden ? null : accountCb.input, toWalletF.el.hidden ? null : toWalletCb.input, walletCb.input, taxF.el.hidden ? null : taxSel, partnerCb.input, descIn, pctF.el.hidden ? null : pctIn].filter(Boolean);
  }

  form.addEventListener('submit', (e) => e.preventDefault());
  form.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      opts.onSubmit({ cont: true });
      return;
    }
    if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      opts.onCancel();
      return;
    }
    if (e.key !== 'Enter' || e.shiftKey || e.altKey || e.defaultPrevented) return;
    if (e.target.tagName === 'BUTTON' || e.target.tagName === 'A') return;
    e.preventDefault();
    const list = order();
    const i = list.findIndex((el) => el === e.target || (el.contains && el.contains(e.target)));
    if (i === list.length - 1) opts.onSubmit({ cont: true });
    else if (i >= 0) {
      const next = list[i + 1];
      if (next === kindSeg.el) kindSeg.focus();
      else next.focus();
    }
  });

  applyKind();
  runner.schedule();
  baseline = fingerprint();

  return {
    el: form,
    collect,
    /** 最後の collect() で誤りのあった欄の名前。 */
    get problems() {
      return problemNames;
    },
    /** 開いたとき（保存して続けたとき）から、入力が変わったか。 */
    get dirty() {
      return fingerprint() !== baseline;
    },
    get previewFailed() {
      return runner.failed;
    },
    focusKind: () => kindSeg.focus(),
    focusDate: () => dateIn.input.focus(),
    /** 金額へ（保存して続けた後・テンプレートの後）。下の方から戻るときに、引き継いだ日付も見えるよう、日付の欄から見せる。 */
    focusAmount() {
      dateF.el.scrollIntoView({ block: 'nearest' });
      amountIn.input.focus();
    },
    setKind: (k) => setKind(k),
    /** サーバーが返した誤り（field 付き）を、その欄の下に出す。欄が見つからなければ偽。 */
    showFieldError(name, message) {
      const f = FIELD_OF[name];
      if (!f) return false;
      f[0].setError(message);
      f[1]().focus();
      return true;
    },
    /** テンプレートの中身を入れる（日付は入れない）。 */
    applyTemplate(body) {
      if (body.kind) setKind(body.kind);
      if (body.amount) amountIn.setValue(body.amount);
      s.amount = body.amount ?? s.amount;
      if (body.kind === 'transfer') {
        toWalletCb.setValue(body.to_wallet);
        s.to_wallet = body.to_wallet || null;
      } else if (body.account) {
        accountCb.setValue(body.account);
        s.account = body.account;
        s.tax_code = body.tax_code ?? null;
        taxSel.value = s.tax_code || '';
        s.business_pct = body.business_pct ?? 100;
        pctIn.value = String(s.business_pct);
      }
      if (body.wallet) {
        walletCb.setValue(body.wallet);
        s.wallet = body.wallet;
      }
      partnerCb.setValue(body.partner || '');
      s.partner = body.partner || '';
      descIn.value = body.description || '';
      s.description = descIn.value;
      s.memo = body.memo || '';
      if (exempt && s.tax_code) detailOpen = true;
      applyKind();
      changed();
    },
    /** 保存した後、種別・日付・口座を残して、ほかを空にする（AC-02.5）。プレビューもすぐ空にする（前の仕訳を残さない）。 */
    resetForNext() {
      s.amount = null;
      s.account = null;
      s.to_wallet = null;
      s.tax_code = null;
      s.partner = '';
      s.description = '';
      s.business_pct = 100;
      s.memo = '';
      amountIn.setValue(null);
      accountCb.setValue(null);
      toWalletCb.setValue(null);
      taxSel.value = '';
      partnerCb.setValue('');
      descIn.value = '';
      pctIn.value = '100';
      Object.values(FIELD_OF).forEach(([f]) => f.setError(''));
      applyKind();
      runner.run();
      baseline = fingerprint();
    },
    /** いまの入力（テンプレートに登録するときに使う）。 */
    snapshot: () => buildInput(),
    /** 「仕訳で入力」に切り替えるときの初期値（最後のプレビューの明細）。 */
    journalSeed: () => ({
      date: s.date,
      description: s.description,
      partner: s.partner,
      lines: lastLines.map((l) => ({ side: l.side, account: l.account, amount: l.amount, tax_code: l.tax_code, memo: l.memo })),
    }),
    /** 表示中のフォームだけがプレビューを送る。 */
    setPreviewActive: (on) => runner.setActive(on),
    destroy() {
      runner.cancel();
      accountCb.destroy();
      walletCb.destroy();
      toWalletCb.destroy();
      partnerCb.destroy();
    },
  };
}
