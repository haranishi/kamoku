// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の値の読み取り（api-inbox.md の「列の対応」）。模擬サーバーの一時の道具で、本物は kamoku-import が持つ。
// 日付: 2026/10/1 2026-10-01 20261001 2026年10月1日 R8.10.1 令和8年10月1日。金額: カンマ・円記号・全角・△ ▲ − の負。

const pad2 = (n) => String(n).padStart(2, '0');

function realDate(y, m, d) {
  const t = new Date(Date.UTC(y, m - 1, d));
  return t.getUTCFullYear() === y && t.getUTCMonth() === m - 1 && t.getUTCDate() === d ? `${y}-${pad2(m)}-${pad2(d)}` : null;
}

function era(name, n, m, d) {
  const y = name === 'R' || name === '令和' ? 2018 + n : 1988 + n;
  const iso = realDate(y, m, d);
  if (!iso) return null;
  // 元号の期間の外の日は読まない（令和元年4月30日など）。
  if ((name === 'R' || name === '令和') && iso < '2019-05-01') return null;
  if ((name === 'H' || name === '平成') && (iso < '1989-01-08' || iso > '2019-04-30')) return null;
  return iso;
}

/** 日付の文字を YYYY-MM-DD にする。読めなければ null。 */
export function parseDate(text) {
  const s = String(text ?? '').normalize('NFKC').trim();
  let m;
  if ((m = /^(\d{4})([/\-.])(\d{1,2})\2(\d{1,2})$/.exec(s))) return realDate(Number(m[1]), Number(m[3]), Number(m[4]));
  if ((m = /^(\d{4})(\d{2})(\d{2})$/.exec(s))) return realDate(Number(m[1]), Number(m[2]), Number(m[3]));
  if ((m = /^(\d{4})年(\d{1,2})月(\d{1,2})日$/.exec(s))) return realDate(Number(m[1]), Number(m[2]), Number(m[3]));
  if ((m = /^([RH])(\d{1,2})\.(\d{1,2})\.(\d{1,2})$/i.exec(s))) return era(m[1].toUpperCase(), Number(m[2]), Number(m[3]), Number(m[4]));
  if ((m = /^(令和|平成)(\d{1,2}|元)年(\d{1,2})月(\d{1,2})日$/.exec(s))) return era(m[1], m[2] === '元' ? 1 : Number(m[2]), Number(m[3]), Number(m[4]));
  return null;
}

/**
 * 金額の欄を読む。空なら { value: null }、読めなければ { error }。
 * @returns {{ value: number|null } | { error: string }}
 */
export function parseAmount(text) {
  let s = String(text ?? '').normalize('NFKC').replace(/[,\s¥\\円]/g, '');
  if (s === '') return { value: null };
  let negative = false;
  if (/^[-−△▲]/.test(s)) {
    negative = true;
    s = s.slice(1);
  } else if (/^\(.*\)$/.test(s)) {
    negative = true;
    s = s.slice(1, -1);
  } else if (/-$/.test(s)) {
    negative = true;
    s = s.slice(0, -1);
  }
  if (s.includes('.')) return { error: '金額に小数点があります。円の整数だけを読めます。' };
  if (!/^\d+$/.test(s)) return { error: `金額「${String(text).trim()}」を読めません。` };
  const value = Number(s);
  return { value: negative ? -value : value };
}

/** 摘要をそろえる（全角の英数字を半角に、連続する空白を1つに、前後の空白を除く）。 */
export function normalizeText(text) {
  return String(text ?? '').normalize('NFKC').replace(/\s+/g, ' ').trim();
}

const WORDS = [
  ['out', ['出金', 'お支払金額', '引出']],
  ['in', ['入金', 'お預り金額', '預入']],
  ['date', ['日付', '年月日', 'ご利用日', '取引日']],
  ['desc', ['摘要', '内容', 'ご利用先', 'お取引内容']],
  ['amount', ['金額']],
];

function role(cell) {
  const c = normalizeText(cell).replace(/ /g, '');
  const hit = WORDS.find(([, words]) => words.some((w) => c.includes(w)));
  return hit ? hit[0] : null;
}

/** 見出しの語から列の対応を推測する（先頭の10行）。見つからなければ null。取引番号の列は推測しない。 */
export function guessMapping(rows) {
  for (let i = 0; i < Math.min(rows.length, 10); i += 1) {
    const roles = rows[i].cells.map(role);
    const find = (r) => roles.indexOf(r);
    const date = find('date');
    if (date < 0) continue;
    let amount;
    if (find('out') >= 0 && find('in') >= 0) amount = { mode: 'split', out_col: find('out'), in_col: find('in') };
    else if (find('amount') >= 0) amount = { mode: 'signed', col: find('amount'), inflow: 'negative' };
    else continue;
    const desc = roles.map((r, j) => (r === 'desc' ? j : -1)).filter((j) => j >= 0);
    return { skip_rows: i + 1, date_col: date, desc_cols: desc, id_col: null, amount };
  }
  return null;
}

/**
 * 1行を列の対応で読む。読めれば { line, date, description, direction, amount, ref }（ref は取引番号）、読めなければ { line, reason }。
 */
export function readRow(row, mapping) {
  const cell = (c) => (Number.isInteger(c) && row.cells[c] !== undefined ? row.cells[c] : '');
  const dateText = cell(mapping.date_col).trim();
  const date = parseDate(dateText);
  if (!date) return { line: row.line, reason: dateText ? `日付「${dateText}」を読めません（実在しない日を含む）。` : '日付が空です。' };
  let direction;
  let amount;
  const a = mapping.amount;
  if (a.mode === 'split') {
    const out = parseAmount(cell(a.out_col));
    const inn = parseAmount(cell(a.in_col));
    if (out.error || inn.error) return { line: row.line, reason: out.error || inn.error };
    if ((out.value ?? 0) < 0 || (inn.value ?? 0) < 0) return { line: row.line, reason: '入金・出金の列に負の金額があります。符号つきの1列の形を選んでください。' };
    const o = out.value || 0;
    const n = inn.value || 0;
    if (o && n) return { line: row.line, reason: '入金と出金の両方に金額があります。' };
    if (!o && !n) return { line: row.line, reason: '入金と出金が、どちらも空か0円です。' };
    direction = o ? 'out' : 'in';
    amount = o || n;
  } else {
    const v = parseAmount(cell(a.col));
    if (v.error) return { line: row.line, reason: v.error };
    if (!v.value) return { line: row.line, reason: '金額が空か0円です。' };
    const positiveIsIn = a.inflow === 'positive';
    direction = v.value > 0 === positiveIsIn ? 'in' : 'out';
    amount = Math.abs(v.value);
  }
  if (amount > 9999999999999) return { line: row.line, reason: '金額が大きすぎます（9,999,999,999,999円まで）。' };
  const description = normalizeText((mapping.desc_cols || []).map(cell).join(' '));
  const ref = Number.isInteger(mapping.id_col) ? normalizeText(cell(mapping.id_col)) || null : null;
  return { line: row.line, date, description, direction, amount, ref };
}
