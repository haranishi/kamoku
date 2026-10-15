// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の期間の選び方（SC-06、AC-08.7）: 「年」「月」「自由な期間」。貸借対照表は日付を1つ、月次推移は年だけ。
// 自由な期間は、年を先に選び、その年の中の日付だけを受け付ける（年をまたぐ選び方は、選ぶ時点で止める。AC-08.8）。
// 最初の1画面を数字に使うため、期間は「‹ 2026年 ▾ ›」の1行にまとめ、欄の上の見出しは出さない
// （名前は、選び方の legend と、各欄の aria-label で読み上げる）。

import { h, uid, addDescribedBy, removeDescribedBy } from '../dom.js';
import { icon } from '../icons.js';
import { createSegmented } from '../components/segmented.js';
import { createDateInput } from '../components/date-input.js';
import { shouldIgnoreKey } from '../ime.js';
import { iconButton, selectInput, button } from '../components/fields.js';
import { checkRange, shiftPeriod } from '../period.js';
import { monthRange } from '../input-parse.js';

const MODES = [
  { value: 'year', label: '年' },
  { value: 'month', label: '月' },
  { value: 'range', label: '自由な期間' },
];

/**
 * @param {{ period: object, years: number[], today: string, dateMode?: boolean, yearOnly?: boolean, onChange: (p: object) => void,
 *           onInvalid?: (bad: boolean) => void }} o
 *   onInvalid: 自由な期間の欄に誤りが出たとき真、消えたとき偽で呼ぶ（表は前の期間のまま残るため）。
 */
export function createPeriodPicker(o) {
  let p = o.period;
  const host = h('div', { class: 'period-controls' });
  const seg = o.dateMode || o.yearOnly ? null : createSegmented({ legend: '期間の選び方', options: MODES, value: p.mode, className: 'period-mode', onChange: (mode) => changeMode(mode) });
  // 「前の年」「次の年」で動いた先の年が候補に無くても、選択欄に出す（無いと、欄が空になった。体験の評価 第3回 E3、2025年・2024年へ戻ったとき）。
  const yearOpts = () => {
    const ys = o.years.includes(p.year) || !Number.isFinite(p.year) ? o.years : [...o.years, p.year].sort((x, y) => x - y);
    return ys.map((y) => ({ value: String(y), label: `${y}年` }));
  };

  function emit(next) {
    p = next;
    o.onChange(next);
  }

  /** 自由な期間の欄に誤りがある間だけ真を知らせる（帳票の画面が「前の期間を表示中」と出す）。変わったときだけ。 */
  let wasInvalid = false;
  function invalid(bad) {
    if (bad === wasInvalid) return;
    wasInvalid = bad;
    if (o.onInvalid) o.onInvalid(bad);
  }

  function changeMode(mode) {
    if (mode === 'year') emit({ mode: 'year', year: p.year, from: `${p.year}-01-01`, to: `${p.year}-12-31` });
    else if (mode === 'month') {
      const month = `${p.year}-${(p.mode === 'month' ? p.month : o.today).slice(5, 7)}`;
      const r = monthRange(month);
      emit({ mode: 'month', year: p.year, month, from: r.from, to: r.to });
    } else {
      // 自由な期間は、いまの期間を初期値にして、日付を直してもらう（まだ問い合わせない）。
      p = { ...p, mode: 'range' };
      paint();
    }
  }

  // 読み上げの名前にはキーの文字を入れず、キーは aria-keyshortcuts で伝える（名前が「前の年（[ キー）」と読まれた）。
  // マウスで指したときの説明（title）には、キーを残す。
  const prevNext = (label) => [
    iconButton({ iconName: 'chevron-left', label: `前の${label}`, title: `前の${label}（[ キー）`, 'aria-keyshortcuts': '[', dataset: { role: 'prev' }, onClick: () => emit(shiftPeriod(p, -1)) }),
    iconButton({ iconName: 'chevron-right', label: `次の${label}`, title: `次の${label}（] キー）`, 'aria-keyshortcuts': ']', dataset: { role: 'next' }, onClick: () => emit(shiftPeriod(p, 1)) }),
  ];

  function yearSelect(onPick) {
    const sel = selectInput({ value: String(p.year), 'aria-label': '年', class: 'period-year', dataset: { role: 'year' } }, yearOpts());
    sel.addEventListener('change', () => onPick(Number(sel.value)));
    return sel;
  }

  // 描き直すと欄が作り直されるので、フォーカスがあった欄と同じ役目の欄へ戻す（キーボードの位置を失わないため）。
  function paint() {
    const role = host.contains(document.activeElement) ? document.activeElement.dataset.role : null;
    draw();
    const back = role ? host.querySelector(`[data-role="${role}"]`) : null;
    if (back) back.focus();
  }

  function draw() {
    // 描き直したら、前の誤りは無い（自由な期間の欄は、下で確かめ直す）。
    invalid(false);
    if (seg) seg.setValue(p.mode);
    if (p.mode === 'date') {
      const date = createDateInput({ value: p.asOf, onChange: (v) => v && v !== p.asOf && emit({ mode: 'date', year: Number(v.slice(0, 4)), asOf: v }) });
      date.input.setAttribute('aria-label', 'いつの時点か');
      date.input.dataset.role = 'date';
      const [prev, next] = prevNext('月末');
      host.replaceChildren(h('div', { class: 'period-row' }, prev, h('div', { class: 'period-date' }, date.input), next, button({ label: '今日', small: true, dataset: { role: 'today' }, onClick: () => emit({ mode: 'date', year: Number(o.today.slice(0, 4)), asOf: o.today }) })));
      return;
    }
    if (p.mode === 'year') {
      const [prev, next] = prevNext('年');
      host.replaceChildren(h('div', { class: 'period-row' }, prev, yearSelect((y) => emit({ mode: 'year', year: y, from: `${y}-01-01`, to: `${y}-12-31` })), next));
      return;
    }
    if (p.mode === 'month') {
      const [prev, next] = prevNext('月');
      const monthSel = selectInput({ value: p.month.slice(5, 7), 'aria-label': '月', class: 'period-month', dataset: { role: 'month' } }, Array.from({ length: 12 }, (_, i) => ({ value: String(i + 1).padStart(2, '0'), label: `${i + 1}月` })));
      const pick = (year, mm) => {
        const month = `${year}-${mm}`;
        const r = monthRange(month);
        emit({ mode: 'month', year, month, from: r.from, to: r.to });
      };
      monthSel.addEventListener('change', () => pick(p.year, monthSel.value));
      host.replaceChildren(h('div', { class: 'period-row' }, prev, yearSelect((y) => pick(y, p.month.slice(5, 7))), monthSel, next));
      return;
    }
    // 自由な期間: 年を選び、その年の中の始めと終わり。
    let year = p.year;
    const err = h('p', { class: 'field-error', role: 'alert', id: uid('period-err') });
    const fromIn = createDateInput({ value: p.from, onChange: () => check() });
    const toIn = createDateInput({ value: p.to, onChange: () => check() });
    fromIn.input.setAttribute('aria-label', '期間の始め');
    toIn.input.setAttribute('aria-label', '期間の終わり');
    fromIn.input.dataset.role = 'from';
    toIn.input.dataset.role = 'to';
    // 誤りがあっても押せるままにする（押した瞬間に使えなくなると、フォーカスが本文の外へ落ちた）。押したら、直す欄へ移す。
    const apply = button({ label: 'この期間で見る', small: true, variant: 'primary', dataset: { role: 'apply' }, onClick: () => submit() });
    const ys = yearSelect((y) => {
      year = y;
      fromIn.setValue(`${y}${(fromIn.value || `${y}-01-01`).slice(4)}`);
      toIn.setValue(`${y}${(toIn.value || `${y}-12-31`).slice(4)}`);
      check();
    });
    /** 誤りと、直す欄。始めが終わりより後なら、始めの欄。 */
    function problem() {
      const [from, to] = [fromIn.value, toIn.value];
      const blank = '期間の始めと終わりを、日付で入れてください。';
      const outside = `${year}年の中の日付にしてください。帳票は1年ずつ作るので、年をまたぐ期間は選べません。`;
      if (!from) return { msg: blank, input: fromIn.input };
      if (!to) return { msg: blank, input: toIn.input };
      if (from.slice(0, 4) !== String(year)) return { msg: outside, input: fromIn.input };
      if (to.slice(0, 4) !== String(year)) return { msg: outside, input: toIn.input };
      const msg = checkRange(from, to);
      return msg ? { msg, input: fromIn.input } : null;
    }
    /** 誤りを出す（欄に印を付け、文を欄の説明として結び付ける。欄の field の setError と同じ形）。null なら消す。 */
    function show(pr) {
      for (const d of [fromIn, toIn]) {
        d.input.removeAttribute('aria-invalid');
        removeDescribedBy(d.input, err.id);
      }
      err.replaceChildren(...(pr ? [icon('alert', { size: 16 }), h('span', null, pr.msg)] : []));
      if (pr) {
        pr.input.setAttribute('aria-invalid', 'true');
        addDescribedBy(pr.input, err.id);
      }
      invalid(Boolean(pr));
    }
    function check() {
      const pr = problem();
      show(pr);
      return pr;
    }
    function submit() {
      fromIn.commit();
      toIn.commit();
      const pr = check();
      if (pr) pr.input.focus();
      else emit({ mode: 'range', year, from: fromIn.value, to: toIn.value });
    }
    for (const d of [fromIn, toIn]) {
      // Enter（変換中を除く）で、この期間を当てる。
      d.input.addEventListener('keydown', (e) => {
        if (shouldIgnoreKey(e) || e.key !== 'Enter') return;
        e.preventDefault();
        submit();
      });
      // 打ち直したら、押し直す前でも誤りを消す（直したのに、押し直すまで文が残っていた）。離れたときに確かめ直す。
      d.input.addEventListener('input', () => {
        if (err.firstChild) show(null);
      });
    }
    host.replaceChildren(h('div', { class: 'period-row period-range' }, ys, h('div', { class: 'period-date' }, fromIn.input), h('span', { class: 'muted' }, '〜'), h('div', { class: 'period-date' }, toIn.input), apply), err);
    check();
  }

  paint();
  const el = h('div', { class: 'period-picker' }, seg ? seg.el : null, host);
  return {
    el,
    /** アドレスから読んだ期間に合わせる。 */
    set(next) {
      p = next;
      paint();
    },
    shift(n) {
      if (p.mode !== 'range') emit(shiftPeriod(p, n));
    },
  };
}

