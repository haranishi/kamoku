// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の(2): 列の対応（SC-04、AC-04.3）。先頭20行の見本と、日付・摘要（複数列可）・金額（入金と出金が別の列／
// 符号つきの1列と向き）・取引番号（任意）・読み飛ばす行の数。文字コードは選び直せる（auto / utf-8 / shift_jis）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { field, selectInput, textInput, button } from '../components/fields.js';
import { createSegmented } from '../components/segmented.js';
import { banner } from '../components/banner.js';
import { columnOptions, columnCount, initialMapping, describeSource, mappingProblems, encodingLabel, delimiterLabel } from '../import-map.js';

const ENCODINGS = [
  { value: 'auto', label: '自動で判定' },
  { value: 'utf-8', label: 'UTF-8' },
  { value: 'shift_jis', label: 'Shift_JIS' },
];

/**
 * @param {{ upload: object, filename: string, encodingChoice: string, onEncoding: (enc: string) => void,
 *           onBack: () => void, onNext: (mapping: object) => void }} o
 */
export function createColumnsStep(o) {
  const up = o.upload;
  const n = columnCount(up);
  const opts = columnOptions(up);
  const start = initialMapping(up);
  const m = start.mapping;
  const colSelect = (value, extra = []) => selectInput({ value: value === null || value === undefined ? '' : String(value) }, [...extra, ...opts.map((x) => ({ value: String(x.value), label: x.label }))]);

  const encSel = selectInput({ value: o.encodingChoice || 'auto', 'aria-label': '文字コード' }, ENCODINGS);
  encSel.addEventListener('change', () => o.onEncoding(encSel.value));
  const facts = h(
    'dl',
    { class: 'import-facts' },
    h('div', null, h('dt', null, 'ファイル'), h('dd', null, o.filename)),
    h('div', null, h('dt', null, '文字コード'), h('dd', { class: 'enc-dd' }, h('span', null, `${encodingLabel(up.encoding)}${o.encodingChoice === 'auto' ? '（自動で判定）' : ''}`), encSel)),
    h('div', null, h('dt', null, '区切り'), h('dd', null, delimiterLabel(up.delimiter))),
    h('div', null, h('dt', null, '行の数'), h('dd', null, `${up.rows}行`)),
  );
  const warnings = (up.warnings || []).length ? banner({ kind: 'warn', title: '読めない文字がありました', body: up.warnings.join(' ') }) : null;

  // ---- 列の対応の欄 ----
  const dateSel = colSelect(m.date_col);
  const dateF = field({ label: '日付の列', control: dateSel, required: true });
  const descBoxes = opts.map((x) => h('label', { class: 'check' }, h('input', { type: 'checkbox', value: String(x.value), checked: m.desc_cols.includes(x.value) }), h('span', null, x.label)));
  const descSet = h('fieldset', { class: 'check-group' }, h('legend', { class: 'field-label' }, '摘要の列（いくつでも。選んだ順に空白でつなぐ）'), h('div', { class: 'check-list' }, descBoxes));
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
  const outF = field({ label: '出金の列', control: outSel });
  const inF = field({ label: '入金の列', control: inSel });
  const colF = field({ label: '金額の列', control: colSel });
  const amountErr = h('p', { class: 'field-error', role: 'alert' });
  const idSel = colSelect(m.id_col, [{ value: '', label: '使わない' }]);
  const idF = field({ label: '取引番号の列（任意）', control: idSel, hint: '金融機関が付けた番号の列があれば選んでください。重複を見分けるいちばん強い手がかりになります。' });
  const skipIn = textInput({ value: String(m.skip_rows), inputmode: 'numeric', class: 'input--num skip-input' });
  const skipF = field({ label: '読み飛ばす行の数', control: h('div', { class: 'affix' }, skipIn, h('span', { class: 'affix-text' }, '行')), labelFor: skipIn, hint: 'ファイルの先頭から、見出しや注意書きの行を数えて入れます。' });

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

  // ---- 見本の表（先頭20行）。選んだ列の見出しに役目の札を付ける ----
  const tableHost = h('div', { class: 'import-sample-host' });
  function paintTable() {
    const cur = current();
    const role = (i) => {
      const tags = [];
      if (cur.date_col === i) tags.push('日付');
      if (cur.desc_cols.includes(i)) tags.push('摘要');
      if (cur.amount.mode === 'split' && cur.amount.out_col === i) tags.push('出金');
      if (cur.amount.mode === 'split' && cur.amount.in_col === i) tags.push('入金');
      if (cur.amount.mode === 'signed' && cur.amount.col === i) tags.push('金額');
      if (cur.id_col === i) tags.push('取引番号');
      return tags;
    };
    const head = h('tr', null, h('th', { scope: 'col', class: 'num' }, '#'), opts.map((x) => h('th', { scope: 'col', class: role(x.value).length ? 'is-mapped' : null }, h('span', { class: 'col-name' }, (up.header && up.header[x.value]) || `${x.value + 1}列目`), role(x.value).map((t) => h('span', { class: 'badge badge--ok col-tag' }, t)))));
    const body = (up.sample || []).slice(0, 20).map((r, i) => h('tr', null, h('td', { class: 'num muted' }, String(i + 1)), opts.map((x) => h('td', { class: role(x.value).length ? 'is-mapped' : null }, String(r[x.value] ?? '')))));
    tableHost.replaceChildren(
      h('div', { class: 'table-wrap', role: 'region', 'aria-label': 'ファイルの先頭の行（見本）', tabindex: '0' }, h('table', { class: 'table sample-table' }, h('caption', null, 'ファイルの先頭の行（見本）。選んだ列には、役目の札が付きます'), h('thead', null, head), h('tbody', null, body))),
    );
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

  const nextBtn = button({ label: '確かめる', variant: 'primary', iconName: 'chevron-right', onClick: () => submit() });
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
  skipIn.addEventListener('input', () => check());

  paintMode();
  const el = h(
    'div',
    { class: 'import-step stack' },
    facts,
    warnings,
    h('p', { class: 'import-source' }, icon(start.source === 'none' ? 'alert' : 'info', { size: 18 }), h('span', null, describeSource(start.source))),
    tableHost,
    form,
    problemsEl,
    h('div', { class: 'import-actions' }, button({ label: '戻る', variant: 'ghost', iconName: 'chevron-left', onClick: () => o.onBack() }), nextBtn),
  );
  return { el, focus: () => dateSel.focus() };
}
