// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 期首残高の入力（初期設定の(3)と、設定の「期首残高」で同じ部品。SC-00、SC-10、api-masters.md の PUT /opening）。
// 資産と負債の科目ごとに残高を入れる。元入金は差額（資産−負債）で、サーバーが入れる。0の行は仕訳に入らない。
// 分からなければ0のままで進めてよい（あとで、ここから見直せる）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { parseAmountExpr, normalizeWidth } from '../input-parse.js';
import { chartCompare } from '../report-tree.js';
import { createDateInput } from '../components/date-input.js';
import { createCombobox } from '../components/combobox.js';
import { field, iconButton } from '../components/fields.js';

const EXCLUDED = new Set(['owner_draw', 'owner_inv']);

/** 残高の欄を読む。空と「0」は0円、それ以外は金額の式（AC-02.3 と同じ読み方）。 */
export function readBalance(text) {
  const t = normalizeWidth(text).replace(/[,\s円¥]/g, '');
  if (t === '' || /^0+$/.test(t)) return { ok: true, value: 0 };
  const r = parseAmountExpr(text);
  return r.ok ? { ok: true, value: r.value } : r;
}

/**
 * @param {{ accounts: any[], opening: { date: string|null, balances: Array<{ account: string, amount: number }> }, defaultDate: string }} o
 * @returns {{ el: HTMLElement, collect: () => ({ date: string, balances: Array<{ account: string, amount: number }> }|null), focus: () => void }}
 */
export function createOpeningForm(o) {
  const byCode = new Map(o.accounts.map((a) => [a.code, a]));
  const usable = o.accounts.filter((a) => a.enabled && (a.kind === 'asset' || a.kind === 'liability') && !EXCLUDED.has(a.code)).sort(chartCompare);
  const given = new Map((o.opening.balances || []).map((b) => [b.account, b.amount]));
  // 口座は全部並べる。口座でない科目は、残高が入っているものだけ並べ、あとは「科目を足す」で足す。
  const codes = [...usable.filter((a) => a.wallet).map((a) => a.code), ...usable.filter((a) => !a.wallet && given.has(a.code)).map((a) => a.code)];
  const rows = new Map();
  const tbody = h('tbody');
  const totals = h('dl', { class: 'opening-totals', 'aria-live': 'polite' });
  const err = h('p', { class: 'field-error', role: 'alert' });

  const date = createDateInput({ value: o.opening.date || o.defaultDate, onError: (m) => dateF.setError(m) });
  const dateF = field({ label: '期首の日付', control: date.el, required: true, hint: 'ふつうは開業日（または、記帳を始める年の1月1日）です。最初の会計年度の中の日付にしてください。' });

  function paintTotals() {
    let assets = 0;
    let liabilities = 0;
    for (const [code, r] of rows) {
      const v = readBalance(r.input.value);
      if (!v.ok) continue;
      if (byCode.get(code).kind === 'asset') assets += v.value;
      else liabilities += v.value;
    }
    const capital = assets - liabilities;
    totals.replaceChildren(
      h('div', null, h('dt', null, '資産の合計'), h('dd', null, h('span', { class: 'num' }, formatYen(assets)), '円')),
      h('div', null, h('dt', null, '負債の合計'), h('dd', null, h('span', { class: 'num' }, formatYen(liabilities)), '円')),
      h('div', { class: 'opening-capital' }, h('dt', null, '元入金（資産−負債）'), h('dd', null, h('span', { class: 'num' }, formatYen(capital)), '円', capital < 0 ? h('span', { class: 'badge badge--warn' }, '債務超過') : null)),
    );
  }

  function addRow(code, amount) {
    const a = byCode.get(code);
    if (!a || rows.has(code)) return null;
    const input = h('input', { type: 'text', class: 'input input--num', inputmode: 'decimal', autocomplete: 'off', value: amount ? formatYen(amount) : '', placeholder: '0', 'aria-label': `${a.name}の残高` });
    input.addEventListener('input', paintTotals);
    input.addEventListener('blur', () => {
      const v = readBalance(input.value);
      if (v.ok) {
        input.value = v.value ? formatYen(v.value) : '';
        input.removeAttribute('aria-invalid');
        err.textContent = '';
      } else {
        input.setAttribute('aria-invalid', 'true');
        err.textContent = `${a.name}: ${v.error}`;
      }
      paintTotals();
    });
    const isCard = a.wallet && a.wallet.type === 'card';
    const remove = a.wallet ? null : iconButton({ iconName: 'trash', label: `${a.name}の行を外す`, onClick: () => { rows.delete(code); tr.remove(); paintTotals(); } });
    const tr = h(
      'tr',
      null,
      h('th', { scope: 'row' }, h('span', { class: 'opening-name' }, a.name), h('span', { class: 'sub' }, a.kind === 'asset' ? '資産' : isCard ? '負債・カードの未払い額' : '負債')),
      h('td', { class: 'num' }, h('div', { class: 'affix' }, input, h('span', { class: 'affix-text' }, '円'))),
      h('td', { class: 'opening-ops' }, remove),
    );
    rows.set(code, { input, tr });
    tbody.append(tr);
    return input;
  }

  codes.forEach((c) => addRow(c, given.get(c) || 0));
  const others = createCombobox({
    items: usable.filter((a) => !a.wallet).map((a) => ({ value: a.code, label: a.name, reading: a.reading, hint: a.kind === 'asset' ? '資産' : '負債' })),
    listLabel: '科目の候補',
    placeholder: '例: うりかけきん（売掛金）',
    onSelect: (item) => {
      if (!item) return undefined;
      const input = addRow(item.value, 0);
      others.setValue(null);
      if (input) setTimeout(() => input.focus(), 0);
      paintTotals();
      return 'stop';
    },
  });
  const othersF = field({ label: '口座でない科目を足す（売掛金・借入金など）', control: others.el, labelFor: others.input });
  paintTotals();

  const el = h(
    'div',
    { class: 'opening-form stack' },
    dateF.el,
    h('div', { class: 'table-wrap' }, h('table', { class: 'table opening-table' }, h('caption', null, '科目ごとの期首の残高'), h('thead', null, h('tr', null, h('th', { scope: 'col' }, '科目'), h('th', { scope: 'col', class: 'num' }, '残高'), h('th', { scope: 'col' }, h('span', { class: 'visually-hidden' }, '操作')))), tbody)),
    othersF.el,
    totals,
    err,
    h('p', { class: 'small muted opening-hint' }, icon('info', { size: 16 }), h('span', null, '分からなければ、0のままで進めてかまいません。元入金は、資産と負債の差額で自動で決まります。')),
  );

  return {
    el,
    /** 送る形。読めない欄があれば理由を出して null。 */
    collect() {
      err.textContent = '';
      if (!date.commit()) {
        date.input.focus();
        return null;
      }
      const balances = [];
      for (const [code, r] of rows) {
        const v = readBalance(r.input.value);
        if (!v.ok) {
          err.textContent = `${byCode.get(code).name}: ${v.error}`;
          r.input.focus();
          return null;
        }
        balances.push({ account: code, amount: v.value });
      }
      return { date: date.value, balances };
    },
    focus: () => {
      const first = rows.values().next().value;
      (first ? first.input : date.input).focus();
    },
    destroy: () => others.destroy(),
  };
}
