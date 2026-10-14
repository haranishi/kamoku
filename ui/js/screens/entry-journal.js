// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 「仕訳で入力」: 行を足せる表（借貸・科目・金額・税区分・メモ）。貸借の差を常に出し、0のときだけ保存できる。
// 税額の欄は置かない。tax_amount を省いて送り、サーバーが金額から出した税額をプレビューに出す（api-entries.md）。

import { h, uid } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { todayIso } from '../input-parse.js';
import { formatYen } from '../format.js';
import { field, textInput, iconButton, button } from '../components/fields.js';
import { createCombobox } from '../components/combobox.js';
import { createDateInput } from '../components/date-input.js';
import { createAmountInput } from '../components/amount-input.js';
import { TAX_LABELS, nextDefaults } from '../account-defaults.js';
import { accountItems, partnerItems } from './entry-options.js';
import { createPreviewRunner } from './entry-run-preview.js';

const TAX_CHOICES = [{ value: '', label: '対象外' }, ...Object.entries(TAX_LABELS).map(([value, label]) => ({ value, label }))];

/**
 * @param {{ accounts: any[], partners: any[], initial?: { date?: string, description?: string, partner?: string, lines?: any[] },
 *           preview: any, api: any, onSubmit: (o: { cont: boolean }) => void, onCancel: () => void,
 *           onDirty?: () => void, onPreviewState?: (failed: boolean) => void, onBalance?: (ok: boolean) => void }} opts
 */
export function createJournalForm(opts) {
  const byCode = new Map(opts.accounts.map((a) => [a.code, a]));
  const items = accountItems(opts.accounts, 'expense');
  const init = opts.initial || {};
  let dirty = false;
  const rows = [];
  const changed = () => {
    dirty = true;
    if (opts.onDirty) opts.onDirty();
    paintBalance();
    runner.schedule();
  };

  const dateIn = createDateInput({ value: init.date || todayIso(), onChange: changed, onError: (m) => dateF.setError(m) });
  const dateF = field({ label: '日付', control: dateIn.el, required: true });
  const partnerCb = createCombobox({ items: partnerItems(opts.partners), allowFree: true, value: init.partner || '', listLabel: '取引先の候補', onInput: changed, onSelect: changed });
  const partnerF = field({ label: '取引先', control: partnerCb.el, labelFor: partnerCb.input });
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
      diff === 0
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
    const row = { id: uid('ln'), side: sideSel, tax: taxSel, account: null, amount: null, memo: null, el: null };
    row.account = createCombobox({
      items,
      value: line.account || null,
      listLabel: '科目の候補',
      onSelect: (item) => {
        const d = nextDefaults({ prev: byCode.get(row.prevAccount) || null, next: byCode.get(item && item.value) || null, tax: taxSel.value || null, pct: 100 });
        taxSel.value = d.tax || '';
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
    taxSel.addEventListener('change', changed);
    const remove = iconButton({ iconName: 'trash', label: 'この行を消す', onClick: () => removeRow(row) });
    row.el = h(
      'tr',
      null,
      h('td', { 'data-label': '借方・貸方' }, sideSel),
      h('td', { 'data-label': '科目', class: 'jl-account' }, row.account.el),
      h('td', { 'data-label': '金額', class: 'num jl-amount' }, row.amount.el),
      h('td', { 'data-label': '税区分', class: 'jl-tax' }, taxSel),
      h('td', { 'data-label': 'メモ', class: 'jl-memo' }, row.memo),
      h('td', { 'data-label': '' }, remove),
    );
    rows.push(row);
    tbody.append(row.el);
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
    (rows[Math.min(i, rows.length - 1)].side).focus();
    changed();
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

  function collect() {
    lineError.textContent = '';
    const problems = [];
    if (!dateIn.commit()) problems.push(dateIn.input);
    rows.forEach((r, i) => {
      r.account.input.removeAttribute('aria-invalid');
      r.amount.input.removeAttribute('aria-invalid');
      if (!r.account.value) {
        r.account.input.setAttribute('aria-invalid', 'true');
        problems.push([r.account.input, `${i + 1}行目の科目を選んでください。`]);
      }
      if (!r.amount.commit()) {
        r.amount.input.setAttribute('aria-invalid', 'true');
        problems.push([r.amount.input, `${i + 1}行目の金額を入れてください（1円以上）。`]);
      }
    });
    const { diff } = sums();
    if (!problems.length && diff !== 0) problems.push([rows[rows.length - 1].amount.input, `貸借が合っていません。差の${formatYen(Math.abs(diff))}円を、どちらかの行に足してください。`]);
    if (problems.length) {
      const first = problems[0];
      if (Array.isArray(first)) {
        lineError.textContent = first[1];
        first[0].focus();
      } else first.focus();
      return null;
    }
    return buildInput();
  }

  const form = h(
    'form',
    { class: 'entry-form', novalidate: true, 'aria-label': '仕訳の入力' },
    h('div', { class: 'entry-grid' }, dateF.el, partnerF.el, descF.el),
    h('div', { class: 'table-wrap jl-wrap', role: 'region', 'aria-label': caption, tabindex: '0' }, table),
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
  dirty = false;
  paintBalance();

  return {
    el: form,
    collect,
    get dirty() {
      return dirty;
    },
    get previewFailed() {
      return runner.failed;
    },
    get balanced() {
      return sums().diff === 0 && rows.length >= 2;
    },
    focusFirst: () => dateIn.input.focus(),
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
