// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の行（inbox-row.js）の欄を作る: 確定する・日付・科目・税区分・取引先・メモ・割合・無視。
// どの欄も、読み上げの名前を「列の名前＋行の名前（日付・摘要・金額）」にし、説明に行の下の文を結ぶ。保存と決まりは inbox-row.js。

import { h } from '../dom.js';
import { taxOptions } from '../account-defaults.js';
import { createCombobox } from '../components/combobox.js';
import { NO_ACCOUNT } from './entry-simple-fields.js';
import { createDateInput } from '../components/date-input.js';
import { textInput } from '../components/fields.js';
import { accountItems, partnerItems } from './entry-options.js';
import { shortDate, taxSelect, PCT_NOTE_ID, ignoreButton } from './inbox-row-parts.js';

/**
 * @param {{ draft: object, kind: 'income'|'expense', accounts: any[], partners: any[], readOnly: boolean, thisYear: string,
 *           colId: (col: string) => string, nameId: string, msgId: string, onDate: (iso: string) => void,
 *           onDateError: (message: string) => void, onAccountSelect: (item: object|null, info: { via: string }) => any,
 *           onIgnore: () => void }} p
 *   draft は作る時点の行（欄の初めの値に使う）。onAccountSelect は、科目の候補の部品の onSelect にそのまま渡す。
 */
export function createRowFields(p) {
  const { draft } = p;
  const labelled = (el, col) => {
    el.setAttribute('aria-labelledby', `${p.colId(col)} ${p.nameId}`);
    el.setAttribute('aria-describedby', p.msgId);
    if (p.readOnly) el.disabled = true;
    return el;
  };

  const check = labelled(h('input', { type: 'checkbox', class: 'cand-check' }), 'cand');
  // 狭い幅の要約の行に出す日付（日付の欄は「くわしく」の中に畳む）。
  const sumDate = h('span', { class: 'sum-date' }, shortDate(draft.date, p.thisYear));
  const date = createDateInput({ value: draft.date, arrowKeys: false, display: (iso) => shortDate(iso, p.thisYear), placeholder: '', inputClass: 'input--date', onError: p.onDateError, onChange: p.onDate });
  labelled(date.input, 'date');
  const account = createCombobox({
    items: accountItems(p.accounts, p.kind, { forEntry: true, keep: [draft.account] }),
    value: draft.account,
    arrowOpens: false,
    listLabel: '科目の候補',
    // 科目は、確定するのに要る欄。任意の取引先・メモ（仮の文字は「取引先」「メモ」）と、文字でも見分けられるようにする。
    placeholder: '科目（必須）',
    noMatch: NO_ACCOUNT,
    onSelect: p.onAccountSelect,
  });
  labelled(account.input, 'account');
  account.input.setAttribute('aria-required', 'true');
  const tax = labelled(taxSelect(taxOptions(p.kind), draft.tax_code), 'tax');
  const partner = createCombobox({ items: partnerItems(p.partners), allowFree: true, arrowOpens: false, value: draft.partner || '', listLabel: '取引先の候補', placeholder: '取引先', noMatch: (text) => `「${text}」は、新しい取引先として足します。` });
  labelled(partner.input, 'partner');
  const memo = labelled(textInput({ value: draft.memo || '', placeholder: 'メモ', maxlength: 200 }), 'memo');
  const pct = draft.direction === 'out' ? labelled(textInput({ value: String(draft.business_pct ?? 100), inputmode: 'numeric', class: 'input--num pct-cell', maxlength: 3 }), 'pct') : null;
  // 割合の欄の説明には、何の割合で残りがどこへ行くか（PCT_NOTE）も結ぶ。
  if (pct) pct.setAttribute('aria-describedby', `${p.msgId} ${PCT_NOTE_ID}`);
  const ignore = ignoreButton(p.onIgnore);
  if (p.readOnly) ignore.el.disabled = true;
  return { check, sumDate, date, account, tax, partner, memo, pct, ignore };
}
