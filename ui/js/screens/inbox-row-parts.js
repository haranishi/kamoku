// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の行（inbox-row.js）の部品: 税区分の選択欄、「くわしく」の開閉、「無視」のボタン、行の下の文、日付の短い書き方。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { shouldIgnoreKey } from '../ime.js';
import { formatSigned, weekdayJa } from '../format.js';
import { isTransitional } from '../account-defaults.js';
import { selectInput, button } from '../components/fields.js';
import { emptyValue } from '../components/money.js';
import { phrase } from '../components/phrase.js';
import { taxDiffers, taxDiffText, taxLabel, taxChoiceLabel, unsetTaxText, directionWarning, stepOption } from './inbox-rules.js';

/** カードの並びで「くわしく」に畳む欄（誤りが出たら、開いて見せる）。 */
export const FOLDED = ['date', 'tax', 'partner', 'memo', 'pct', 'business_pct'];
// サーバーの誤りの欄の名前（error.field）を、行の欄の名前にする。知らない名前は行全体の誤り。
const FIELD_OF = { tax_code: 'tax', business_pct: 'business_pct', date: 'date', partner: 'partner', memo: 'memo', account: 'account' };
export const fieldOf = (field) => FIELD_OF[field] || 'row';

/** 手で選んでいない空の税区分の仮の値。「対象外」を濃い字で出すと、決まった値に見えたため。 */
export const TAX_UNSET = '__unset';

/**
 * 割合の列の説明（取引の登録の画面の欄の補足と同じ文）。表の見出しの title と、各行の割合の欄の見えない説明に使う
 * （見出しが「割合」だけで、何の割合か、残りがどこへ行くかが分からなかった。E2 第2回）。
 */
export const PCT_NOTE = '仕事に使った分の割合。残りは事業主貸に入ります。';
export const PCT_NOTE_ID = 'inbox-pct-note';

/**
 * 「10/01 (木)」。年が違うときだけ「2025/12/30 (火)」。ほかの画面の日付（「2026-10-01 (木)」）と同じく曜日を付ける
 * （狭い列なので、今年なら年を省く）。
 */
export function shortDate(iso, thisYear) {
  const day = iso.slice(0, 4) === thisYear ? `${iso.slice(5, 7)}/${iso.slice(8, 10)}` : iso.replace(/-/g, '/');
  return `${day} (${weekdayJa(iso)})`;
}

/**
 * 税区分の選択欄。先頭に、選べない仮の値（科目で決まる）を隠して持つ。
 * 上下の矢印は、値の切り替えにする（表の行の移動に取られて、キーボードで値を変えられなかった）。端末の標準に任せないのは、
 * Mac の Chrome では一覧が開くだけで、一覧が開かない環境もあったため。行の移動は、Enter と Tab で残る。
 * @param {Array<{ value: string, label: string }>} choices
 */
export function taxSelect(choices, value) {
  const sel = selectInput({ value: value || '' }, choices.map((x) => ({ value: x.value, label: taxChoiceLabel(x.value) })));
  sel.prepend(h('option', { value: TAX_UNSET, disabled: true, hidden: true }, '科目で決まる'));
  sel.addEventListener('keydown', (e) => {
    if (shouldIgnoreKey(e)) return;
    if ((e.key !== 'ArrowDown' && e.key !== 'ArrowUp') || e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
    // 端でも止めて、表の行の移動（inbox-table.js。止めた印 defaultPrevented を見る）に回さない。
    e.preventDefault();
    const i = stepOption([...sel.options], sel.selectedIndex, e.key === 'ArrowDown' ? 1 : -1);
    if (i < 0) return;
    sel.selectedIndex = i;
    sel.dispatchEvent(new Event('change', { bubbles: true }));
  });
  return sel;
}

/**
 * 税区分の欄の見せ方。手で選んでいない空の税区分は、仮の値で薄く出す（科目があれば「対象外（科目で決まる）」）。
 * @param {HTMLSelectElement} sel
 * @param {{ code: string|null, unset: boolean, hasAccount: boolean, account: object|null, kind: 'income'|'expense' }} p
 */
export function paintTaxSelect(sel, { code, unset, hasAccount, account, kind }) {
  const placeholder = unsetTaxText({ account, hasAccount, kind });
  sel.options[0].textContent = placeholder.label;
  sel.value = unset ? TAX_UNSET : code || '';
  sel.classList.toggle('is-placeholder', unset);
  sel.title = unset ? placeholder.title : taxLabel(code);
}

// 表の並び（1080px以上）で、行の下の文を、その欄の下に寄せる側。税区分から右の欄（税区分・取引先・メモ・割合）は右、ほかは左。
const RIGHT_FIELDS = new Set(['tax', 'partner', 'memo', 'business_pct']);
const sideOf = (field) => (RIGHT_FIELDS.has(field) ? 'right' : 'left');

/**
 * 行の下に出す文の並び: 欄の誤り、向きと逆の科目の注意、科目の既定と違う税区分、経過措置の案内（inbox-rules.js）。
 * どの文も、どの欄の話か（data-for）と、表で寄せる側（data-side）を持つ（幅1440で、税区分の話と日付の話が1つの塊に見え、
 * 欄から離れていた。見た目の採点 第2回）。カードの並びでは、科目の欄のすぐ下に並ぶ（screens-inbox-cards.css）。
 * @param {{ errors: Record<string, string>, account: object|null, draft: object, kind: string, exempt: boolean }} p
 */
export function rowMessages({ errors, account, draft, kind, exempt }) {
  const parts = [];
  for (const [field, text] of Object.entries(errors)) if (text) parts.push(h('p', { class: 'field-error', dataset: { field, for: field, side: sideOf(field) } }, icon('alert', { size: 16 }), h('span', null, text)));
  const warn = directionWarning(account, draft.direction);
  if (warn) parts.push(rowNote('warn', warn, 'account'));
  if (taxDiffers({ account, kind, exempt, tax: draft.tax_code })) parts.push(rowNote('info', taxDiffText(account, kind, exempt), 'tax'));
  if (!exempt && isTransitional(draft.tax_code)) parts.push(rowNote('info', '日付は、課税仕入れの日にしてください。商品の引渡し・役務の提供が完了した日です。経過措置の割合は、この行の日付で決まります。', 'date'));
  return parts;
}

/**
 * カードの並び（入れ物が1080px未満）で、税区分・取引先・メモ・割合・日付の欄を畳む「くわしく」の開閉。
 * 表のとき（1080px以上）は CSS で隠し、全部の欄を出したままにする（キーボードの流れを変えない）。
 * 文字は開くと「閉じる」に変わる（矢印の向きだけでは分かりにくかった。E2 第2回）。読み上げの名前は「くわしく: 09/16 サンプルストア
 * 1,910円」のように、見える文字に行の名前を足す（25行の「くわしく」を区別できるように。E4 第2回）。開閉は aria-expanded でも伝える。
 * @param {{ controls: string[], onToggle: (open: boolean) => void }} o
 */
export function moreToggle(o) {
  const text = h('span', null, 'くわしく');
  const btn = h('button', { type: 'button', class: 'btn btn--ghost btn--sm more-btn', 'aria-expanded': 'false', 'aria-controls': o.controls.join(' ') }, text, icon('chevron-down', { size: 16 }));
  let open = false;
  let rowName = '';
  const paint = () => {
    const word = open ? '閉じる' : 'くわしく';
    text.textContent = word;
    btn.setAttribute('aria-expanded', String(open));
    btn.setAttribute('aria-label', `${word}: ${rowName}`);
  };
  btn.addEventListener('click', () => o.onToggle(!open));
  paint();
  return {
    el: btn,
    set(v) {
      open = v;
      paint();
    },
    /** 行の名前（「09/18 デモ電器 7,470円」）が変わったら呼ぶ。 */
    name(t) {
      rowName = t;
      paint();
    },
  };
}

/**
 * 無視するボタン。記号だけだと何が起きるか分からなかったので、文字を添える。読み上げの名前には、行の名前を足す。
 * キー（x）は名前に入れず、aria-keyshortcuts で伝える（名前に「（x）」が混ざっていた。E4 第2回）。
 */
export function ignoreButton(onClick) {
  const btn = button({ label: '無視', iconName: 'ban', small: true, variant: 'ghost', className: 'ignore-btn', title: 'x キーでも無視できます', 'aria-keyshortcuts': 'x', onClick });
  return {
    el: btn,
    /** 行の名前（「09/18 デモ電器 7,470円」）が変わったら呼ぶ。 */
    name(text) {
      btn.setAttribute('aria-label', `無視する: ${text}`);
    },
  };
}

/**
 * 行の下の案内の1つ。kind が 'warn' なら注意の印（三角）と注意の色、'info' なら案内の印。色だけに頼らず、印と文字で示す。
 * 最初の一文を見出し（太字）にし、続きは小さく出す（長い注意が同じ強さで何行も続き、要点を拾いにくかった。見た目の採点 第2回）。
 * @param {'warn'|'info'} kind
 * @param {string} text
 * @param {string} field どの欄の話か（account・tax・date）
 */
export function rowNote(kind, text, field) {
  const cut = text.indexOf('。') + 1;
  const head = cut > 0 ? text.slice(0, cut) : text;
  const rest = cut > 0 ? text.slice(cut) : '';
  const words = (t) => (t.length > 20 ? phrase(t) : t);
  return h(
    'p',
    { class: ['field-hint', 'row-note', `row-note--${kind}`], dataset: { note: kind, for: field, side: sideOf(field) } },
    icon(kind === 'warn' ? 'alert' : 'info', { size: 16 }),
    h('span', { class: 'note-body' }, h('span', { class: 'note-head' }, words(head)), rest ? h('span', { class: 'note-detail' }, words(rest)) : null),
  );
}

/**
 * 欄の並び（Tabの順と読み上げの順）を、見た目の順にそろえる。CSSの格子で見た目だけを動かすと、「くわしく」を開いて Tab を
 * 押したときに、見えている日付を飛ばしたため。
 * - table（入れ物が1080px以上）: inbox-row.js の COLUMNS の順（科目の後ろに、カードでだけ見える「くわしく」）。
 * - cards（600〜1079px）: 2段のカード。摘要・金額／科目・くわしく・無視、開くと日付から下。
 * - stack（600px未満）: 摘要の右に金額と「無視」を縦に積み、科目と「くわしく」を次の段に並べる（1件が3段で高かった）。
 */
const CELL_ORDER = {
  table: ['cand', 'date', 'desc', 'amount', 'account', 'more', 'tax', 'partner', 'memo', 'pct', 'ops'],
  cards: ['cand', 'desc', 'amount', 'account', 'more', 'ops', 'date', 'tax', 'partner', 'memo', 'pct'],
  stack: ['cand', 'desc', 'amount', 'ops', 'account', 'more', 'date', 'tax', 'partner', 'memo', 'pct'],
};

/** @param {'table'|'cards'|'stack'} mode */
export function orderCells(tr, mode) {
  if (tr.dataset.order === mode) return;
  for (const col of CELL_ORDER[mode]) tr.append(tr.querySelector(`:scope > td[data-col="${col}"]`));
  tr.dataset.order = mode;
}

/**
 * 行の <tr>。欄の並びは inbox-row.js の COLUMNS と同じで、科目の後ろに、狭い幅でだけ見える「くわしく」の欄を挟む。
 * 摘要の欄には、摘要と、小さな行（狭い幅の「確定する」の札・日付・口座・「科目なし」の札）と、読み上げだけの行の名前（p.name）を入れる。
 * 「科目なし」の札は、摘要の上の1行から、小さな行の末尾へ移した（25行のほとんどに同じ札が1行ずつ並び、摘要と金額より先に目に入り、
 * 行も高くなった。見た目の採点 第2回）。必須であることは、科目の欄の「科目（必須）」でも示す。
 * 「確定する」の札は、カードの並びで、印を付けた行の本文の側に出す（チェックの下の小さな字では、状態として弱かった）。
 * 印のチェック（名前は「確定する …」）が状態を伝えるので、札は読み上げない。金額の単位は、表では見出し「金額（円）」にあり、
 * 見出しの無いカードの並びでだけ、金額の後ろに出す。
 */
export function rowElement(p) {
  const { draft } = p;
  const cell = (col, label, ...children) => h('td', { class: `c-${col}`, id: p.cellId(col), dataset: { col }, 'data-label': label }, children);
  const signed = formatSigned(draft.direction === 'in' ? draft.amount : -draft.amount);
  return h(
    'tr',
    { class: 'inbox-row is-folded', dataset: { id: draft.id, dir: draft.direction, order: 'table' } },
    cell('cand', '確定する', h('label', { class: 'cand' }, p.check, h('span', { class: 'cand-text' }, '確定する'))),
    cell('date', '日付', p.date),
    cell(
      'desc',
      '',
      h('span', { class: 'desc-text', id: p.descId }, draft.description || '（摘要なし）'),
      h('span', { class: 'sub' }, h('span', { class: 'cand-badge', 'aria-hidden': 'true' }, '確定する'), p.sumDate, h('span', { class: 'keep' }, draft.wallet_name)),
      p.name,
    ),
    cell('amount', '金額', h('span', { class: ['amount', `amount--${draft.direction}`] }, signed, h('span', { class: 'amount-unit' }, '円')), h('span', { class: 'sub dir-text' }, draft.direction === 'in' ? '入金' : '出金')),
    cell('account', '科目', p.account),
    h('td', { class: 'c-more', dataset: { col: 'more' } }, p.more),
    cell('tax', '税区分', p.tax),
    cell('partner', '取引先', p.partner),
    cell('memo', 'メモ', p.memo),
    cell('pct', '事業の割合', p.pct ? h('div', { class: 'affix' }, p.pct, h('span', { class: 'affix-text' }, '%')) : emptyValue()),
    cell('ops', '', p.ignore),
  );
}
