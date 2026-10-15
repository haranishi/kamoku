// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 「仕訳で入力」: 行を足せる表（借貸・科目・金額・税区分・メモ）。貸借の差を常に出し、0のときだけ保存できる。
// 税額の欄は置かない。tax_amount を省いて送り、サーバーが金額から出した税額をプレビューに出す（api-entries.md）。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { todayIso } from '../input-parse.js';
import { formatYen, formatDate } from '../format.js';
import { field, textInput, iconButton, button } from '../components/fields.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { createAmountInput } from '../components/amount-input.js';
import { TAX_LABELS, nextDefaults } from '../account-defaults.js';
import { accountItems, partnerItems } from './entry-options.js';
import { createPreviewRunner } from './entry-run-preview.js';
import { fieldNote, newPartnerLabel, NO_ACCOUNT } from './entry-simple-fields.js';
import { phrase } from '../components/phrase.js';

const TAX_CHOICES = [{ value: '', label: '対象外' }, ...Object.entries(TAX_LABELS).map(([value, label]) => ({ value, label }))];

/**
 * @param {{ accounts: any[], partners: any[], initial?: { date?: string, description?: string, partner?: string, lines?: any[] },
 *           preview: any, api: any, labelledBy?: string, onSubmit: (o: { cont: boolean }) => void, onCancel: () => void,
 *           onDirty?: () => void, onPreviewState?: (failed: boolean) => void, onBalance?: (ok: boolean) => void }} opts
 */
export function createJournalForm(opts) {
  const byCode = new Map(opts.accounts.map((a) => [a.code, a]));
  const init = opts.initial || {};
  // 訂正で、元の仕訳が口座の親の科目（現金など）を使っているときは、その科目を候補に残す。
  const items = accountItems(opts.accounts, 'expense', { forEntry: true, keep: (init.lines || []).map((l) => l.account) });
  let baseline = '';
  const rows = [];
  const changed = () => {
    if (opts.onDirty) opts.onDirty();
    paintBalance();
    syncPartnerNote();
    runner.schedule();
  };

  const dateIn = createDateInput({
    value: init.date || todayIso(),
    onChange: (v) => {
      // 締めた日以前の日付なら、欄の下に出す（かんたん入力と同じ）。
      const m = v && opts.dateProblem ? opts.dateProblem(v) : '';
      if (m) dateF.setError(m);
      changed();
    },
    onError: (m) => dateF.setError(m),
  });
  const dateF = field({ label: '日付', control: dateIn.el, required: true });
  // 取引先は、かんたん入力と同じ決まり（Enterで先頭の候補。打った名前は、一覧の最後の行を選んだときだけ新しい取引先）。
  const partnerCb = createCombobox({
    items: partnerItems(opts.partners),
    allowFree: true,
    createLabel: newPartnerLabel,
    value: init.partner || '',
    listLabel: '取引先の候補',
    onInput: () => {
      if (!partnerCb.value) partnerF.setError('');
      changed();
    },
    onSelect: () => {
      partnerF.setError('');
      changed();
    },
  });
  const partnerNote = fieldNote(() => partnerCb.input);
  const partnerF = field({ label: '取引先', control: partnerCb.el, labelFor: partnerCb.input, after: partnerNote.el });
  function syncPartnerNote() {
    const p = partnerCb.item;
    partnerNote.set(p && p.isNew ? `新しい取引先: ${p.label}` : '', { plain: true });
  }
  const descIn = textInput({ value: init.description || '', placeholder: '例: 開業時の備品の購入' });
  descIn.addEventListener('input', changed);
  const descF = field({ label: '摘要', control: descIn, className: 'span-2' });

  const tbody = h('tbody');
  const caption = '仕訳の明細';
  const table = h(
    'table',
    { class: 'table table--stack jl-table' },
    h('caption', null, caption),
    h(
      'thead',
      null,
      h('tr', null, ['借方・貸方', '科目', '金額', '税区分', 'メモ'].map((t, i) => h('th', { scope: 'col', class: i === 2 ? 'num' : null }, t)), h('th', { scope: 'col' }, h('span', { class: 'visually-hidden' }, '行の操作'))),
    ),
    tbody,
  );
  const balance = h('p', { class: 'jl-balance', 'aria-live': 'polite' });
  const lineError = h('p', { class: 'field-error', role: 'alert' });
  const addBtn = button({ label: '行を足す', iconName: 'plus', small: true, onClick: () => addRow({}, true) });

  function sums() {
    let debit = 0;
    let credit = 0;
    for (const r of rows) {
      const v = r.amount.value || 0;
      if (r.side.value === 'debit') debit += v;
      else credit += v;
    }
    return { debit, credit, diff: debit - credit };
  }

  function paintBalance() {
    const { debit, credit, diff } = sums();
    balance.replaceChildren(
      h('span', null, `借方 ${formatYen(debit)}円`),
      h('span', null, `貸方 ${formatYen(credit)}円`),
      // 何も入れていない（借方も貸方も0円）ときは、合っているとは言わない（開いた直後に「貸借が合っています」と出た。体験の評価 第3回 E4）。
      debit === 0 && credit === 0
        ? h('span', { class: 'badge badge--muted' }, '金額を入れると、貸借の差を確かめます')
        : diff === 0
          ? h('span', { class: 'badge badge--ok' }, icon('check'), '貸借が合っています')
          : h('span', { class: 'badge badge--warn' }, icon('alert'), `差 ${formatYen(Math.abs(diff))}円（${diff > 0 ? '貸方' : '借方'}が足りません）`),
    );
    if (opts.onBalance) opts.onBalance(diff === 0 && rows.length >= 2);
  }

  function addRow(line, focus = false) {
    const { diff } = sums();
    const sideSel = h('select', { class: 'select', 'aria-label': '借方か貸方' }, h('option', { value: 'debit' }, '借方'), h('option', { value: 'credit' }, '貸方'));
    sideSel.value = line.side || (diff > 0 ? 'credit' : 'debit');
    const taxSel = h('select', { class: 'select', 'aria-label': '税区分' }, TAX_CHOICES.map((o) => h('option', { value: o.value }, o.label)));
    taxSel.value = line.tax_code || '';
    // 狭い幅（640px未満）では、長い税区分（経過措置の…）が欄に入り切らない。選んだ名前を、欄の下にも全部出す（CSS が狭い幅だけ見せる）。
    // 読み上げは選択欄の値で全部読まれるので、こちらは読ませない。
    const taxFull = h('span', { class: 'jl-tax-full', 'aria-hidden': 'true' });
    const syncTaxFull = () => {
      const label = taxSel.selectedOptions[0] ? taxSel.selectedOptions[0].label : '';
      taxFull.textContent = label.length > 12 ? label : '';
    };
    const row = { id: uid('ln'), side: sideSel, tax: taxSel, account: null, amount: null, memo: null, el: null };
    row.account = createCombobox({
      items,
      value: line.account || null,
      listLabel: '科目の候補',
      noMatch: NO_ACCOUNT,
      enterNeedsMatch: true,
      onSelect: (item) => {
        // 候補を選んだら、その欄の誤りの表示をすぐ消す（行の誤りの文も、この欄のものなら消す）。
        if (item) clearProblem(row.account.input);
        const d = nextDefaults({ prev: byCode.get(row.prevAccount) || null, next: byCode.get(item && item.value) || null, tax: taxSel.value || null, pct: 100 });
        taxSel.value = d.tax || '';
        syncTaxFull();
        row.prevAccount = item ? item.value : null;
        changed();
      },
    });
    row.prevAccount = line.account || null;
    row.account.input.setAttribute('aria-label', '科目');
    row.amount = createAmountInput({ value: line.amount ?? (focus && diff !== 0 ? Math.abs(diff) : null), onChange: changed });
    row.amount.input.setAttribute('aria-label', '金額');
    row.memo = textInput({ value: line.memo || '', 'aria-label': 'メモ' });
    row.memo.addEventListener('input', changed);
    sideSel.addEventListener('change', changed);
    taxSel.addEventListener('change', () => {
      syncTaxFull();
      changed();
    });
    syncTaxFull();
    const remove = iconButton({ iconName: 'trash', label: 'この行を消す', onClick: () => removeRow(row) });
    row.remove = remove;
    row.el = h(
      'tr',
      null,
      h('td', { 'data-label': '借方・貸方', class: 'jl-side' }, sideSel),
      h('td', { 'data-label': '科目', class: 'jl-account' }, row.account.el),
      h('td', { 'data-label': '金額', class: 'num jl-amount' }, row.amount.el),
      h('td', { 'data-label': '税区分', class: 'jl-tax' }, taxSel, taxFull),
      h('td', { 'data-label': 'メモ', class: 'jl-memo' }, row.memo),
      h('td', { 'data-label': '', class: 'jl-del' }, remove),
    );
    rows.push(row);
    tbody.append(row.el);
    renumber();
    if (focus) sideSel.focus();
    changed();
    return row;
  }

  function removeRow(row) {
    if (rows.length <= 2) {
      lineError.textContent = '明細は2行以上が要ります。行を消す前に、別の行を足してください。';
      return;
    }
    const i = rows.indexOf(row);
    rows.splice(i, 1);
    row.account.destroy();
    row.el.remove();
    renumber();
    (rows[Math.min(i, rows.length - 1)].side).focus();
    changed();
  }

  // 欄の名前に行の番号を入れる（読み上げで、2行とも「科目」「金額」と聞こえ、どの行か分からなかった）。足す・消すたびに振り直す。
  function renumber() {
    rows.forEach((r, i) => {
      const n = `${i + 1}行目`;
      r.side.setAttribute('aria-label', `${n} 借方か貸方`);
      r.account.input.setAttribute('aria-label', `${n} 科目`);
      r.amount.input.setAttribute('aria-label', `${n} 金額`);
      r.tax.setAttribute('aria-label', `${n} 税区分`);
      r.memo.setAttribute('aria-label', `${n} メモ`);
      r.remove.setAttribute('aria-label', `${n}を消す`);
      r.remove.title = `${n}を消す`;
    });
  }

  function buildInput() {
    if (!dateIn.value || rows.length < 2) return null;
    const lines = [];
    for (const r of rows) {
      if (!r.account.value || !r.amount.value) return null;
      lines.push({ account: r.account.value, side: r.side.value, amount: r.amount.value, tax_code: r.tax.value || null, memo: r.memo.value });
    }
    return { kind: 'journal', date: dateIn.value, description: descIn.value, partner: partnerCb.value, lines };
  }

  const runner = createPreviewRunner({ api: opts.api, preview: opts.preview, build: buildInput, onFailedChange: opts.onPreviewState, idleText: 'すべての行に科目と金額を入れると、ここに税額つきの仕訳が出ます。' });

  // 行の誤りの文が、どの欄のものか（その欄で候補を選んだら、文も消すため）。
  let problemEl = null;
  function clearProblem(el) {
    el.removeAttribute('aria-invalid');
    if (problemEl === el) {
      lineError.textContent = '';
      problemEl = null;
    }
  }

  // 最後の検査で誤りのあった欄の名前（保存の帯の一言「2か所を直してください：…」に使う）。
  let problemNames = [];

  function collect() {
    lineError.textContent = '';
    problemEl = null;
    const problems = [];
    const names = [];
    if (!dateIn.commit()) {
      problems.push(dateIn.input);
      names.push('日付');
    }
    // 候補に無い打ちかけの取引先は、黙って新しい取引先にしない（かんたん入力と同じ）。
    const name = partnerCb.input.value.trim();
    const unsure = Boolean(name && !partnerCb.item);
    partnerF.setError(unsure ? `「${name}」は候補にありません。候補から選ぶか、一覧の最後の行で足してください。` : '');
    if (unsure) {
      problems.push(partnerCb.input);
      names.push('取引先');
    }
    rows.forEach((r, i) => {
      r.account.input.removeAttribute('aria-invalid');
      r.amount.input.removeAttribute('aria-invalid');
      if (!r.account.value) {
        r.account.input.setAttribute('aria-invalid', 'true');
        problems.push([r.account.input, `${i + 1}行目の科目を選んでください。`]);
        names.push(`${i + 1}行目の科目`);
      }
      if (!r.amount.commit()) {
        r.amount.input.setAttribute('aria-invalid', 'true');
        problems.push([r.amount.input, `${i + 1}行目の金額を入れてください（1円以上）。`]);
        names.push(`${i + 1}行目の金額`);
      }
    });
    const { diff } = sums();
    if (!problems.length && diff !== 0) {
      problems.push([rows[rows.length - 1].amount.input, `貸借が合っていません。差の${formatYen(Math.abs(diff))}円を、どちらかの行に足してください。`]);
      names.push('貸借の差');
    }
    problemNames = names;
    if (problems.length) {
      const first = problems[0];
      if (Array.isArray(first)) {
        lineError.textContent = first[1];
        problemEl = first[0];
        first[0].focus();
      } else first.focus();
      return null;
    }
    return buildInput();
  }

  // 入力の指紋。開いたときから変わったかを見る（離れる前の確認に使う）。打ちかけの文字（日付・科目・金額）も入れる。
  function fingerprint() {
    const typed = (cb) => (cb.item ? '' : cb.input.value.trim());
    const dateText = dateIn.input.value.trim();
    const dateRaw = dateIn.value && (dateText === dateIn.value || dateText === formatDate(dateIn.value)) ? '' : dateText;
    const lines = rows.map((r) => [r.side.value, r.account.value, typed(r.account), r.amount.value, r.amount.value === null ? r.amount.input.value.trim() : '', r.tax.value, r.memo.value]);
    return JSON.stringify([dateIn.value, dateRaw, partnerCb.input.value.trim(), descIn.value, lines]);
  }

  const named = opts.labelledBy ? { 'aria-labelledby': opts.labelledBy } : { 'aria-label': '仕訳の入力' };
  // 借方・貸方を初めて見る人への1行（2026-10-15 の体験の評価 E1・E4。会計の一般的な説明だけで、税の判断は書かない）。
  const help = h('p', { class: 'field-hint jl-help', id: uid('jl-help') }, phrase('取引を借方と貸方に分けて書き、借方の合計と貸方の合計を同じにします（例: 現金で消耗品を買ったら、借方に消耗品費、貸方に現金）。'));
  const form = h(
    'form',
    { class: 'entry-form', novalidate: true, ...named },
    h('div', { class: 'entry-grid' }, dateF.el, partnerF.el, descF.el),
    help,
    h('div', { class: 'table-wrap jl-wrap', role: 'region', 'aria-label': caption, 'aria-describedby': help.id, tabindex: '0' }, table),
    h('div', { class: 'jl-foot' }, addBtn, balance),
    lineError,
  );
  form.addEventListener('submit', (e) => e.preventDefault());
  form.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      opts.onSubmit({ cont: true });
    } else if (e.key === 'Escape' && !e.defaultPrevented) {
      e.preventDefault();
      opts.onCancel();
    } else if (e.key === 'Enter' && !e.defaultPrevented && !e.shiftKey && e.target.tagName !== 'BUTTON') {
      // Enterは次の欄へ。最後の行のメモでは、貸借が合っていれば保存の合図、合っていなければ行を足す。
      e.preventDefault();
      const fields = [...form.querySelectorAll('input:not([type="hidden"]), select')].filter((x) => !x.closest('[hidden]'));
      const i = fields.indexOf(e.target);
      if (i === fields.length - 1) {
        if (sums().diff === 0) opts.onSubmit({ cont: true });
        else addRow({}, true);
      } else if (i >= 0) fields[i + 1].focus();
    }
  });

  (init.lines && init.lines.length >= 2 ? init.lines : [{ side: 'debit' }, { side: 'credit' }]).forEach((l) => addRow(l));
  paintBalance();
  baseline = fingerprint();

  return {
    el: form,
    collect,
    /** 最後の collect() で誤りのあった欄の名前。 */
    get problems() {
      return problemNames;
    },
    /** 開いたときから、入力が変わったか。 */
    get dirty() {
      return fingerprint() !== baseline;
    },
    get previewFailed() {
      return runner.failed;
    },
    get balanced() {
      return sums().diff === 0 && rows.length >= 2;
    },
    focusFirst: () => dateIn.input.focus(),
    focusDate: () => dateIn.input.focus(),
    /** 訂正で開いたとき: 1行目の金額へ（直すことの多い欄）。 */
    focusAmount: () => rows[0].amount.input.focus(),
    showFieldError(name, message) {
      lineError.textContent = message;
      if (name === 'date') dateIn.input.focus();
      return true;
    },
    setPreviewActive: (on) => runner.setActive(on),
    destroy() {
      runner.cancel();
      partnerCb.destroy();
      rows.forEach((r) => r.account.destroy());
    },
  };
}
