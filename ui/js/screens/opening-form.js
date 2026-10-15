// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 期首残高の入力（初期設定の(3)と、設定の「期首残高」で同じ部品。SC-00、SC-10、api-masters.md の PUT /opening）。
// 資産と負債の科目ごとに残高を入れる。元入金は差額（資産−負債）で、サーバーが入れる。0の行は仕訳に入らない。
// 分からなければ0のままで進めてよい（あとで、ここから見直せる）。

import { h, uid, addDescribedBy, removeDescribedBy } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatYen } from '../format.js';
import { parseAmountExpr, normalizeWidth } from '../input-parse.js';
import { chartCompare } from '../report-tree.js';
import { createDateInput } from '../components/date-input.js';
import { createCombobox } from '../components/combobox.js';
import { field, iconButton } from '../components/fields.js';
import { money } from '../components/money.js';
import { phrase } from '../components/phrase.js';

const EXCLUDED = new Set(['owner_draw', 'owner_inv']);
const NEGATIVE = 'マイナスの金額は入れられません（0円は可）。';

/**
 * 残高の欄を読む。空と「0」は0円、それ以外は金額の式（AC-02.3 と同じ読み方）。式の答えが0円でもよい。
 * マイナスは断る。そのときの文は「0円は可」と添える（金額の欄の決まり「1円以上に」のままだと、すぐ下の
 * 「分からなければ、0のままで進めてかまいません」と食い違って見えた）。
 */
export function readBalance(text) {
  const t = normalizeWidth(text).replace(/[,\s円¥]/g, '');
  if (t === '' || /^0+$/.test(t)) return { ok: true, value: 0 };
  const r = parseAmountExpr(text);
  if (r.ok) return { ok: true, value: r.value };
  // 答えがマイナスなら、0から引いた式は1円以上として読める。答えが0円なら、1を足した式がちょうど1円になる。
  if (parseAmountExpr(`0-(${t})`).ok) return { ok: false, error: NEGATIVE };
  const plusOne = parseAmountExpr(`${t}+1`);
  if (plusOne.ok && plusOne.value === 1) return { ok: true, value: 0 };
  return r;
}

/**
 * 文字の欄の Enter で次の欄へ進み、最後の欄の Enter は submit（進むボタンを押したのと同じ）にする（取引の登録と同じ決まり。
 * 初期設定の文字の欄で Enter が何もしなかった）。変換中の Enter は何もしない。ボタンやリンクの上の Enter は、そのまま押す。
 * submit が無ければ、最後の欄の Enter では何もしない。
 * @param {HTMLElement} box 欄を包む要素
 * @param {() => Array<HTMLElement|{ el: HTMLElement, focus: () => void }>} order Enter で進む順の欄（見えている欄だけ）
 * @param {(() => void)|undefined} submit
 */
export function enterAdvances(box, order, submit) {
  box.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if (e.key !== 'Enter' || e.shiftKey || e.altKey || e.ctrlKey || e.metaKey || e.defaultPrevented) return;
    if (e.target.tagName === 'BUTTON' || e.target.tagName === 'A') return;
    const list = order();
    const i = list.findIndex((x) => (x.el || x) === e.target || (x.el || x).contains(e.target));
    if (i < 0 || (i === list.length - 1 && !submit)) return;
    e.preventDefault();
    if (i === list.length - 1) submit();
    else list[i + 1].focus();
  });
}

/**
 * @param {{ accounts: any[], opening: { date: string|null, balances: Array<{ account: string, amount: number }> }, defaultDate: string,
 *           onSubmit?: () => void }} o
 *   onSubmit: 最後の残高の欄で Enter を押したとき（初期設定の「保存して次へ」）。無ければ、最後の欄の Enter では何もしない。
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
  const err = h('p', { class: 'field-error opening-err', role: 'alert', id: uid('opening-err') });

  /** 残高の欄に誤りを出す（印と、表の下の文。文は欄の説明として結び付ける）。message が空なら消す。 */
  function rowError(input, message) {
    err.textContent = message || '';
    if (message) {
      input.setAttribute('aria-invalid', 'true');
      addDescribedBy(input, err.id);
    } else {
      input.removeAttribute('aria-invalid');
      removeDescribedBy(input, err.id);
    }
  }

  const date = createDateInput({ value: o.opening.date || o.defaultDate, onError: (m) => dateF.setError(m) });
  // 開業日との違い（初期設定では、1段階目の開業日と、3段階目のこの日付の両方を聞く）と、受け付ける範囲を書く。
  const dateF = field({ label: '期首の日付', control: date.el, required: true, clearOnInput: true, className: 'opening-date', hint: phrase('期首の日付は、残高を入れる日です。ふつうは開業日と同じです。開業日と同じ年の日付にしてください。') });

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
    // 円は、ほかの画面と同じ書き方（小さい「円」、数字との間に空白なし。money()）。
    totals.replaceChildren(
      h('div', null, h('dt', null, '資産の合計'), h('dd', null, money(assets))),
      h('div', null, h('dt', null, '負債の合計'), h('dd', null, money(liabilities))),
      h('div', { class: 'opening-capital' }, h('dt', null, '元入金（資産−負債）'), h('dd', null, money(capital), capital < 0 ? h('span', { class: 'badge badge--warn' }, '債務超過') : null)),
    );
  }

  function addRow(code, amount) {
    const a = byCode.get(code);
    if (!a || rows.has(code)) return null;
    const input = h('input', { type: 'text', class: 'input input--num', inputmode: 'decimal', autocomplete: 'off', value: amount ? formatYen(amount) : '', placeholder: '0', 'aria-label': `${a.name}の残高` });
    input.addEventListener('input', () => {
      // 打ち直したら、この欄の誤りを消す（離れたときに、もう一度確かめる）。
      if (input.getAttribute('aria-invalid') === 'true') rowError(input, '');
      paintTotals();
    });
    input.addEventListener('blur', () => {
      const v = readBalance(input.value);
      if (v.ok) {
        input.value = v.value ? formatYen(v.value) : '';
        if (input.getAttribute('aria-invalid') === 'true') rowError(input, '');
      } else {
        rowError(input, `${a.name}: ${v.error}`);
      }
      paintTotals();
    });
    const isCard = a.wallet && a.wallet.type === 'card';
    const remove = a.wallet ? null : iconButton({ iconName: 'trash', label: `${a.name}の行を外す`, onClick: () => { rows.delete(code); tr.remove(); paintTotals(); } });
    const tr = h(
      'tr',
      null,
      h('th', { scope: 'row' }, h('span', { class: 'opening-name keep' }, a.name), h('span', { class: 'sub' }, phrase(a.kind === 'asset' ? '資産' : isCard ? '負債・カードの未払い額' : '負債'))),
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
  const othersF = field({ label: '口座でない科目を足す（売掛金・借入金など）', control: others.el, labelFor: others.input, className: 'opening-others' });
  paintTotals();

  const el = h(
    'div',
    { class: 'opening-form stack' },
    dateF.el,
    h('div', { class: 'table-wrap opening-wrap' }, h('table', { class: 'table opening-table' }, h('caption', null, '科目ごとの期首の残高'), h('thead', null, h('tr', null, h('th', { scope: 'col' }, '科目'), h('th', { scope: 'col', class: 'num' }, '残高'), h('th', { scope: 'col' }, h('span', { class: 'visually-hidden' }, '操作')))), tbody)),
    othersF.el,
    totals,
    err,
    h('p', { class: 'small muted opening-hint' }, icon('info', { size: 16 }), h('span', null, phrase('分からなければ、0のままで進めてかまいません。元入金は、資産と負債の差額で自動で決まります。'))),
  );

  // 期首の日付と残高の欄の Enter で、次の残高の欄へ進む（「科目を足す」で足した行も、表の順に入る）。最後の欄の Enter は onSubmit。
  enterAdvances(el, () => [date.input, ...[...rows.values()].map((r) => r.input)], o.onSubmit);

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
          rowError(r.input, `${byCode.get(code).name}: ${v.error}`);
          r.input.focus();
          return null;
        }
        balances.push({ account: code, amount: v.value });
      }
      return { date: date.value, balances };
    },
    /**
     * サーバーの断りを、関係する欄の下に出し、その欄へフォーカスを移す（期首の日付が年度の外、など）。
     * 出せたら真。出せない断り（欄の無いもの）は偽を返すので、呼ぶ側が通知で知らせる。
     */
    showError(e) {
      if (!e || !e.field) return false;
      if (e.field === 'date') {
        dateF.setError(e.message);
        date.input.focus();
        return true;
      }
      const m = /^balances\[(\d+)\]/.exec(e.field);
      const entry = m ? [...rows.entries()][Number(m[1])] : null;
      if (!entry) return false;
      rowError(entry[1].input, `${byCode.get(entry[0]).name}: ${e.message}`);
      entry[1].input.focus();
      return true;
    },
    focus: () => {
      const first = rows.values().next().value;
      (first ? first.input : date.input).focus();
    },
    destroy: () => others.destroy(),
  };
}
