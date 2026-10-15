// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込の列の対応（api-inbox.md の mapping）を、画面で選びやすい形にする補助。DOMを使わないので Node で試せる。
// 初期値は、その口座に覚えさせた対応（saved）→ サーバーの推測（guess）→ 決まった形 の順。

import { formatYen } from './format.js';

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
 * 列の選択肢。名前は「3列目 お支払金額」と短くし（選択欄の中で切れないように）、見本の最初の値は example に分けて、
 * 選択欄の下の補足に出す。見出しが無いファイルは「3列目」だけ。
 * @returns {Array<{ value: number, label: string, name: string, example: string }>}
 */
export function columnOptions(upload) {
  const n = columnCount(upload);
  const header = upload.header || [];
  const first = (upload.sample || []).find((r) => r.some((c) => String(c).trim())) || [];
  return Array.from({ length: n }, (_, i) => {
    const name = String(header[i] ?? '').trim();
    const value = String(first[i] ?? '').trim();
    const example = value && value !== name ? value.slice(0, 24) : '';
    return { value: i, label: name ? `${i + 1}列目 ${name}` : `${i + 1}列目`, name, example };
  });
}

/** 列の見本の値（先頭の行から、空でないものを count 個）。狭い幅の「列ごとの一覧」に出す。 */
export function columnExamples(upload, col, count = 3) {
  const header = upload.header || [];
  const name = String(header[col] ?? '').trim();
  return (upload.sample || [])
    .map((r) => String(r[col] ?? '').trim())
    .filter((v) => v && v !== name)
    .slice(0, count);
}

/** 見本の値がどれも数（桁区切りや符号つきを含む）なら真。見本の表で、その列を右に寄せる。 */
export function isNumberColumn(upload, col) {
  const values = columnExamples(upload, col, 20);
  return values.length > 0 && values.every((v) => /^[-+−△▲]?[\d,，]+(\.\d+)?$/.test(v));
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
 * 「確かめる」の後に「戻る」で(2)へ戻ったときの初期値。直前に自分で選んだ対応をそのまま使う
 * （覚えた対応や推測を入れ直すと、自分で変えた列が消えた。2026-10-15 の体験の評価 E2）。
 * @returns {{ mapping: object, source: 'kept' }}
 */
export function keptMapping(m) {
  return { mapping: clone(m), source: 'kept' };
}

/**
 * 推測の説明の文（画面の(2)の上に出す）。
 * @param {'saved'|'guess'|'none'|'kept'} source
 */
export function describeSource(source) {
  if (source === 'kept') return '直前に選んだ列の対応のままです。直したら、もう一度「確かめる」を押してください。';
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

/**
 * 日付の列に選んだ列が日付らしくないときの注意。取込の確認（POST /imports/preview）の応えの行から数える（日付の読み方は、
 * 取込の確認と同じサーバーの読み方だけを使い、画面に別の読み方を足さない）。サーバーは行を読むとき日付を最初に確かめるので、
 * 日付が読めない行の理由は「日付」で始まる（kamoku-import の mapping.rs の read_row）。読める行が半分に届かなければ文を返す。
 * 2段階目で日付の列に摘要の列を選んでも、「確かめる」を押すまで何も出なかった（E2 第2回）。
 * @param {Array<{ status: string, reason?: string|null }>} rows
 * @returns {string} 注意の文（無ければ空）
 */
export function dateColumnWarning(rows) {
  const total = rows.length;
  const bad = rows.filter((r) => r.status === 'invalid' && /^日付/.test(r.reason || '')).length;
  if (!total || (total - bad) * 2 >= total) return '';
  return `この列は日付に読めない行が多いようです（${total}行のうち${bad}行）。日付の列を選び直してください。`;
}

/** 件数の文（「新規 300件・重複 10件・読めない 2件」）。 */
export function countsText(counts) {
  return `新規 ${counts.new}件・重複 ${counts.duplicate}件・読めない ${counts.invalid}件`;
}

const breaks = (cells) => (cells || []).reduce((n, c) => n + (String(c).match(/\r\n|\r|\n/g) || []).length, 0);

/**
 * 見本の各行の、ファイルの行番号（1始まり）。確かめの画面の「ファイルの○行目」（サーバーの line）と同じ数え方にする
 * （見本を1から数えていたので、同じ行が2つの画面で1と2になった）。サーバーは、推測した読み飛ばしの行（guess.skip_rows）の
 * 後ろから見本を出す。引用符の中の改行（1つの欄が2行にまたがる）は数える。空の行はサーバーが表から外すので、ここでは
 * 分からない（空の行があるファイルでは、その後ろの番号が、空の行の数だけ小さくなる）。
 * @returns {number[]}
 */
export function sampleLines(upload) {
  let line = (upload.guess ? upload.guess.skip_rows : 0) + 1 + breaks(upload.header);
  return (upload.sample || []).map((r) => {
    const at = line;
    line += 1 + breaks(r);
    return at;
  });
}

/**
 * 見本の行のうち、いまの「読み飛ばす行の数」で読み飛ばす行か（見本の i 行目）。数が読めないときは、どれも読み飛ばさない。
 * 見本は、推測した読み飛ばしの行の後ろから始まるので、推測より多く読み飛ばすと、見本の先頭の行が読み飛ばされる。
 */
export function sampleSkipped(upload, skipRows, i) {
  const start = upload.guess ? upload.guess.skip_rows : 0;
  return Number.isInteger(skipRows) && start + i < skipRows;
}

/**
 * 取り込む行（新規と、「この行も取り込む」を選んだ重複）の期間と、出金・入金の件数と合計。重複・読めない行は数えない。
 * @param {Array<{ line: number, status: string, date: string|null, direction: string|null, amount: number|null }>} rows
 * @param {Set<number>} force 選んだ重複の行番号
 */
export function importSummary(rows, force = new Set()) {
  const take = rows.filter((r) => r.status === 'new' || (r.status === 'duplicate' && force.has(r.line)));
  const dates = take.map((r) => r.date).filter(Boolean).sort();
  const sum = (dir) => take.filter((r) => r.direction === dir).reduce((a, r) => ({ count: a.count + 1, total: a.total + (r.amount || 0) }), { count: 0, total: 0 });
  return { count: take.length, from: dates[0] || null, to: dates[dates.length - 1] || null, out: sum('out'), in: sum('in') };
}

/**
 * 「取り込む40件: 2026-09-01〜2026-09-30・出金 37件 126,640円・入金 3件 330,718円」。0件の向きは書かない。
 * 表が先頭の500行だけ（truncated）なら、表の分だと書き添える（件数はファイル全体の数と合わないため）。
 * @param {ReturnType<typeof importSummary>} s
 */
export function summaryText(s, truncated = false) {
  if (!s.count) return '';
  const parts = [s.from === s.to ? s.from : `${s.from}〜${s.to}`];
  if (s.out.count) parts.push(`出金 ${formatYen(s.out.count)}件 ${formatYen(s.out.total)}円`);
  if (s.in.count) parts.push(`入金 ${formatYen(s.in.count)}件 ${formatYen(s.in.total)}円`);
  return `${truncated ? '表に出している行のうち、' : ''}取り込む${formatYen(s.count)}件: ${parts.join('・')}`;
}
