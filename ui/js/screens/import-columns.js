// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の(2): 列の対応（SC-04、AC-04.3）。先頭20行の見本と、日付・摘要（複数列可）・金額（入金と出金が別の列／
// 符号つきの1列と向き）・取引番号（任意）・読み飛ばす行の数。文字コードは選び直せる（auto / utf-8 / shift_jis）。

import { h, addDescribedBy } from '../dom.js';
import { icon } from '../icons.js';
import { field, selectInput, textInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { banner } from '../components/banner.js';
import { columnOptions, columnCount, initialMapping, keptMapping, describeSource, mappingProblems, encodingLabel, delimiterLabel, dateColumnWarning } from '../import-map.js';
import { sampleView } from './import-sample.js';
import { phrase } from '../components/phrase.js';

const ENCODINGS = [
  { value: 'auto', label: '自動で判定' },
  { value: 'utf-8', label: 'UTF-8' },
  { value: 'shift_jis', label: 'Shift_JIS' },
];

/**
 * @param {{ upload: object, filename: string, encodingChoice: string, kept?: object|null, onEncoding: (enc: string) => void,
 *           onBack: () => void, onNext: (mapping: object) => void, preview?: (mapping: object) => Promise<{ rows: any[] }> }} o
 *   kept: (3)から「戻る」で来たときの、直前に選んだ対応（あれば、覚えた対応や推測より優先する）。
 *   preview: 取込の確認を送る（日付の列を選び直したときに、日付として読めるかを確かめるため。取り込みはしない）。
 */
export function createColumnsStep(o) {
  const up = o.upload;
  const n = columnCount(up);
  const opts = columnOptions(up);
  const start = o.kept ? keptMapping(o.kept) : initialMapping(up);
  const m = start.mapping;
  const colSelect = (value, extra = []) => selectInput({ value: value === null || value === undefined ? '' : String(value) }, [...extra, ...opts.map((x) => ({ value: String(x.value), label: x.label }))]);
  // 選んだ列の見本の値は、選択欄の中ではなく、すぐ下の補足の行に出す（選択欄の中に入れると、幅で切れた）。
  const examples = [];
  const withExample = (f) => {
    const sel = f.control;
    const line = h('p', { class: 'field-hint col-example', id: `${sel.id}-ex` });
    const paint = () => {
      const o = opts.find((x) => String(x.value) === sel.value);
      line.textContent = o && o.example ? `例: ${o.example}` : '';
    };
    sel.after(line);
    addDescribedBy(sel, line.id);
    examples.push(paint);
    paint();
    return f;
  };

  // 文字コードは選択欄1つで見せる。自動のときは、判定した文字コードを選択肢に添える（「自動で判定（Shift_JIS）」）。前は判定の結果の文字と
  // 「自動で判定」の選択欄が隣り合い、同じことを2回言っていた（見た目の採点 第2回）。自分で選んだときは、その文字コードで読んだので、
  // 自動の選択肢には何も添えない（自動ならどう判定するかは、この応えからは分からない）。
  const auto = o.encodingChoice === 'auto' || !o.encodingChoice;
  const encSel = selectInput({ value: o.encodingChoice || 'auto', 'aria-label': '文字コード' }, ENCODINGS.map((x) => (x.value === 'auto' && auto ? { ...x, label: `自動で判定（${encodingLabel(up.encoding)}）` } : x)));
  encSel.addEventListener('change', () => o.onEncoding(encSel.value));
  const facts = h(
    'dl',
    { class: 'import-facts' },
    h('div', null, h('dt', null, 'ファイル'), h('dd', null, o.filename)),
    h('div', null, h('dt', null, '文字コード'), h('dd', { class: 'enc-dd' }, encSel)),
    h('div', null, h('dt', null, '区切り'), h('dd', null, delimiterLabel(up.delimiter))),
    h('div', null, h('dt', null, '行の数'), h('dd', null, `${up.rows}行`)),
  );
  const warnings = (up.warnings || []).length ? banner({ kind: 'warn', title: '読めない文字がありました', body: up.warnings.join(' ') }) : null;

  // ---- 列の対応の欄 ----
  const dateSel = colSelect(m.date_col);
  const dateF = withExample(field({ label: '日付の列', control: dateSel, required: true }));
  // 日付の列を選び直したら、その場で日付として読めるかを確かめ、読めない行が多ければ欄の下に出す（import-map.js の dateColumnWarning）。
  const dateWarn = h('p', { class: 'field-hint col-warn', id: `${dateSel.id}-warn`, 'aria-live': 'polite' });
  dateSel.parentElement.append(dateWarn);
  addDescribedBy(dateSel, dateWarn.id);
  let dateSeq = 0;
  async function checkDates() {
    const my = ++dateSeq;
    dateWarn.replaceChildren();
    const cur = current();
    if (!o.preview || mappingProblems(cur, n).length) return;
    try {
      const res = await o.preview(cur);
      const text = my === dateSeq ? dateColumnWarning(res.rows || []) : '';
      if (text) dateWarn.replaceChildren(icon('alert', { size: 16 }), h('span', null, phrase(text)));
    } catch {
      // 確かめられなくても（期限切れなど）、「確かめる」で同じ理由が出る。
    }
  }
  dateSel.addEventListener('change', () => checkDates());
  const descBoxes = opts.map((x) => h('label', { class: 'check' }, h('input', { type: 'checkbox', value: String(x.value), checked: m.desc_cols.includes(x.value) }), h('span', { class: 'keep' }, x.label)));
  const descSet = h('fieldset', { class: 'check-group' }, h('legend', { class: 'field-label' }, phrase('摘要の列（いくつでも選べます。選んだ順に、空白でつなぎます）')), h('div', { class: 'check-list' }, descBoxes));
  const modeSeg = createSegmented({
    legend: '金額の形',
    options: [
      { value: 'split', label: '入金と出金が別の列' },
      { value: 'signed', label: '符号つきの1列' },
    ],
    value: m.amount.mode,
    onChange: () => paintMode(),
  });
  const outSel = colSelect(m.amount.mode === 'split' ? m.amount.out_col : Math.min(2, n - 1));
  const inSel = colSelect(m.amount.mode === 'split' ? m.amount.in_col : Math.min(3, n - 1));
  const colSel = colSelect(m.amount.mode === 'signed' ? m.amount.col : Math.min(2, n - 1));
  const inflowSeg = createSegmented({
    legend: '入金の向き',
    options: [
      { value: 'positive', label: '正の数が入金（銀行の明細に多い）' },
      { value: 'negative', label: '正の数が出金（カードの明細に多い）' },
    ],
    value: m.amount.mode === 'signed' ? m.amount.inflow : 'positive',
    className: 'seg--wrap',
  });
  const outF = withExample(field({ label: '出金の列', control: outSel }));
  const inF = withExample(field({ label: '入金の列', control: inSel }));
  const colF = withExample(field({ label: '金額の列', control: colSel }));
  const amountErr = h('p', { class: 'field-error', role: 'alert' });
  const idSel = colSelect(m.id_col, [{ value: '', label: '使わない' }]);
  const idF = withExample(field({ label: '取引番号の列（任意）', control: idSel, hint: phrase('金融機関が付けた番号の列があれば選んでください。重複を見分ける、いちばん確かな手がかりです。') }));
  const skipIn = textInput({ value: String(m.skip_rows), inputmode: 'numeric', class: 'input--num skip-input' });
  const skipF = field({ label: '読み飛ばす行の数', control: h('div', { class: 'affix' }, skipIn, h('span', { class: 'affix-text' }, '行')), labelFor: skipIn, hint: phrase('ファイルの先頭から、見出しや注意書きの行を数えて入れます。') });

  function paintMode() {
    const split = modeSeg.value === 'split';
    outF.el.hidden = !split;
    inF.el.hidden = !split;
    colF.el.hidden = split;
    inflowSeg.el.hidden = split;
    paintTable();
  }

  function current() {
    const num = (sel) => (sel.value === '' ? null : Number(sel.value));
    const amount = modeSeg.value === 'split' ? { mode: 'split', out_col: num(outSel), in_col: num(inSel) } : { mode: 'signed', col: num(colSel), inflow: inflowSeg.value };
    const skip = Number(skipIn.value.trim());
    return {
      skip_rows: skipIn.value.trim() === '' ? NaN : skip,
      date_col: num(dateSel),
      desc_cols: descBoxes.map((l) => l.querySelector('input')).filter((c) => c.checked).map((c) => Number(c.value)),
      id_col: num(idSel),
      amount,
    };
  }

  // ---- 見本（先頭20行）。選んだ列に役目の札を付ける。広い幅は表、狭い幅は列ごとの一覧（import-sample.js） ----
  const tableHost = h('div', { class: 'import-sample-host' });
  function paintTable() {
    tableHost.replaceChildren(...sampleView(up, opts, current()));
    examples.forEach((paint) => paint());
  }

  const problemsEl = h('div');
  function check() {
    const probs = mappingProblems(current(), n);
    dateF.setError((probs.find((p) => p.field === 'date_col') || {}).message || '');
    skipF.setError((probs.find((p) => p.field === 'skip_rows') || {}).message || '');
    amountErr.textContent = (probs.find((p) => p.field === 'amount') || {}).message || '';
    idF.setError((probs.find((p) => p.field === 'id_col') || {}).message || '');
    return probs;
  }

  // 進む向きの印は、文字の後ろに置く（戻るの印は前）。
  const nextBtn = button({ label: '確かめる', variant: 'primary', onClick: () => submit() });
  nextBtn.append(icon('chevron-right', { size: 18 }));
  function submit() {
    const probs = check();
    if (probs.length) {
      const first = { date_col: dateSel, skip_rows: skipIn, amount: modeSeg.value === 'split' ? outSel : colSel, id_col: idSel }[probs[0].field];
      if (first) first.focus();
      return;
    }
    o.onNext(current());
  }

  const form = h(
    'form',
    { class: 'import-map', novalidate: true, 'aria-label': '列の対応' },
    h('div', { class: 'map-grid' }, dateF.el, skipF.el, descSet, idF.el),
    h('div', { class: 'map-amount' }, modeSeg.el, h('div', { class: 'map-grid' }, outF.el, inF.el, colF.el), inflowSeg.el, amountErr),
  );
  form.addEventListener('submit', (e) => e.preventDefault());
  form.addEventListener('change', () => {
    paintTable();
    check();
  });
  // 読み飛ばす行の数は、打つそばから見本に映す（読み飛ばす行に印が付く。import-sample.js）。
  skipIn.addEventListener('input', () => {
    check();
    paintTable();
  });

  paintMode();
  const el = h(
    'div',
    { class: 'import-step stack' },
    facts,
    warnings,
    h('p', { class: 'import-source' }, icon(start.source === 'none' ? 'alert' : 'info', { size: 18 }), h('span', null, phrase(describeSource(start.source)))),
    tableHost,
    form,
    problemsEl,
    // 画面の下に付いてくる帯（screens-import.css）。data-bottom-bar で、通知がこの帯の上へ持ち上がる。
    h('div', { class: 'import-actions', 'data-bottom-bar': true }, button({ label: '戻る', variant: 'ghost', iconName: 'chevron-left', onClick: () => o.onBack() }), nextBtn),
  );
  return { el, focus: () => dateSel.focus() };
}
