// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の列の対応（api-inbox.md の mapping）を、画面で選びやすい形にする補助。DOMを使わないので Node で試せる。
// 初期値は、その口座に覚えさせた対応（saved）→ サーバーの推測（guess）→ 決まった形 の順。

/** 文字コードの表示名。 */
export function encodingLabel(code) {
  return { 'utf-8': 'UTF-8', utf8: 'UTF-8', shift_jis: 'Shift_JIS', auto: '自動で判定' }[code] || String(code || '不明');
}

/** 区切りの記号の表示名。 */
export function delimiterLabel(d) {
  return { ',': 'カンマ', '\t': 'タブ', ';': 'セミコロン' }[d] || `「${d}」`;
}

/** 列の数（見出しと見本の行のうち、いちばん多い欄の数）。 */
export function columnCount(upload) {
  const widths = [(upload.header || []).length, ...(upload.sample || []).map((r) => r.length)];
  return Math.max(1, ...widths);
}

/**
 * 列の選択肢の名前。見出しがあれば見出しの文字、無ければ「列3」。見本の最初の値を添える。
 * @returns {Array<{ value: number, label: string }>}
 */
export function columnOptions(upload) {
  const n = columnCount(upload);
  const header = upload.header || [];
  const first = (upload.sample || []).find((r) => r.some((c) => String(c).trim())) || [];
  return Array.from({ length: n }, (_, i) => {
    const name = String(header[i] ?? '').trim();
    const example = String(first[i] ?? '').trim();
    const base = name ? `${i + 1}列目「${name}」` : `${i + 1}列目`;
    return { value: i, label: example && example !== name ? `${base}（例: ${example.slice(0, 16)}）` : base };
  });
}

/** 推測できなかったときの初期値。日付は1列目、摘要は2列目、金額は3列目の符号つき（正が入金）。 */
export function fallbackMapping(upload) {
  const n = columnCount(upload);
  const col = (i) => Math.min(i, n - 1);
  return { skip_rows: upload.header ? 1 : 0, date_col: 0, desc_cols: n > 1 ? [col(1)] : [], id_col: null, amount: { mode: 'signed', col: col(2), inflow: 'positive' } };
}

/**
 * 列の対応の初期値と、その出どころ。
 * @returns {{ mapping: object, source: 'saved'|'guess'|'none' }}
 */
export function initialMapping(upload) {
  const n = columnCount(upload);
  const fits = (m) => m && [m.date_col, ...(m.desc_cols || []), m.amount.col, m.amount.out_col, m.amount.in_col].every((c) => c === undefined || c === null || c < n);
  if (fits(upload.saved)) return { mapping: clone(upload.saved), source: 'saved' };
  if (fits(upload.guess)) return { mapping: clone(upload.guess), source: 'guess' };
  return { mapping: fallbackMapping(upload), source: 'none' };
}

function clone(m) {
  return { ...m, desc_cols: [...(m.desc_cols || [])], id_col: m.id_col ?? null, amount: { ...m.amount } };
}

/**
 * 推測の説明の文（画面の(2)の上に出す）。
 * @param {'saved'|'guess'|'none'} source
 */
export function describeSource(source) {
  if (source === 'saved') return 'この口座で前に使った列の対応を入れました。違っていたら直してください。';
  if (source === 'guess') return '見出しの言葉から、列の対応を推測しました。見本と見比べて確かめてください。';
  return '見出しから列を推測できませんでした。日付・摘要・金額の列を選んでください。';
}

/**
 * 選んだ対応の誤り（画面の欄の下に出す）。誤りが無ければ空の配列。
 * @returns {Array<{ field: string, message: string }>}
 */
export function mappingProblems(m, n) {
  const out = [];
  const inRange = (c) => Number.isInteger(c) && c >= 0 && c < n;
  if (!inRange(m.date_col)) out.push({ field: 'date_col', message: '日付の列を選んでください。' });
  if (!Number.isInteger(m.skip_rows) || m.skip_rows < 0) out.push({ field: 'skip_rows', message: '読み飛ばす行の数は、0以上の整数にしてください。' });
  if (m.amount.mode === 'split') {
    if (!inRange(m.amount.out_col) || !inRange(m.amount.in_col)) out.push({ field: 'amount', message: '出金と入金の列を選んでください。' });
    else if (m.amount.out_col === m.amount.in_col) out.push({ field: 'amount', message: '出金と入金に、同じ列が選ばれています。別々の列を選ぶか、「符号つきの1列」にしてください。' });
  } else if (!inRange(m.amount.col)) out.push({ field: 'amount', message: '金額の列を選んでください。' });
  const amountCols = m.amount.mode === 'split' ? [m.amount.out_col, m.amount.in_col] : [m.amount.col];
  if (amountCols.includes(m.date_col)) out.push({ field: 'date_col', message: '日付と金額に、同じ列が選ばれています。' });
  if (m.id_col !== null && m.id_col !== undefined && !inRange(m.id_col)) out.push({ field: 'id_col', message: '取引番号の列を選び直してください。' });
  return out;
}

/** 件数の文（「新規 300件・重複 10件・読めない 2件」）。 */
export function countsText(counts) {
  return `新規 ${counts.new}件・重複 ${counts.duplicate}件・読めない ${counts.invalid}件`;
}
