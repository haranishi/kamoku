// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 模擬サーバーのCSVの読み書き（一時の道具。本物は kamoku-import と kamoku-app が持つ）。
// 文字コードの判定、RFC 4180 の読み取り（行の番号つき）、書き出し（式の実行を防ぐ処理、BOM、Shift_JIS）。

import { ApiFail } from './compose.mjs';

const REPLACEMENT = '�';

function isValidUtf8(bytes) {
  try {
    new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    return true;
  } catch {
    return false;
  }
}

/**
 * バイト列を文字にする。choice は auto / utf-8 / shift_jis。
 * @returns {{ text: string, encoding: 'utf-8'|'shift_jis', warnings: string[] }}
 */
export function decode(bytes, choice = 'auto') {
  if (bytes.includes(0)) {
    throw new ApiFail(400, 'invalid_csv', 'CSVの中に文字ではないデータがあります。表計算ソフトの形式ではなく、CSVで保存したファイルを選んでください。', null);
  }
  const bom = bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf;
  const useUtf8 = choice === 'utf-8' || (choice === 'auto' && isValidUtf8(bytes));
  const text = useUtf8 ? new TextDecoder('utf-8').decode(bom ? bytes.subarray(3) : bytes) : new TextDecoder('shift_jis').decode(bytes);
  const bad = [...text].filter((c) => c === REPLACEMENT).length;
  const warnings = bad ? [`正しく読めない文字が ${bad}か所ありました。文字コードを選び直すと、読める場合があります。`] : [];
  return { text, encoding: useUtf8 ? 'utf-8' : 'shift_jis', warnings };
}

function countOutsideQuotes(line, ch) {
  let inQuote = false;
  let n = 0;
  for (const c of line) {
    if (c === '"') inQuote = !inQuote;
    else if (c === ch && !inQuote) n += 1;
  }
  return n;
}

/** 区切りの記号を選ぶ。最初の数行で、引用符の外に一番多く出てくる記号。 */
export function detectDelimiter(text) {
  const lines = text.split(/\r\n|\n|\r/).filter((l) => l.trim()).slice(0, 10);
  let best = ',';
  let bestScore = -1;
  for (const d of [',', '\t', ';']) {
    const counts = lines.map((l) => countOutsideQuotes(l, d));
    const score = Math.max(0, ...counts);
    if (score > bestScore) {
      best = d;
      bestScore = score;
    }
  }
  return best;
}

/**
 * CSVを読む。行は、その行が始まった物理的な行の番号（1始まり）を持つ。空の行は飛ばす。
 * @returns {Array<{ line: number, cells: string[] }>}
 */
export function parseCsv(text, delimiter) {
  const rows = [];
  let cells = [];
  let cell = '';
  let inQuote = false;
  let line = 1;
  let rowLine = 1;
  let quoteStart = 1;
  const endCell = () => {
    cells.push(cell);
    cell = '';
  };
  const endRow = () => {
    endCell();
    if (cells.some((c) => c.trim() !== '')) rows.push({ line: rowLine, cells: cells.map((c) => c.replace(/[ \t]+$/, '')) });
    cells = [];
  };
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (inQuote) {
      if (c === '"' && text[i + 1] === '"') {
        cell += '"';
        i += 1;
      } else if (c === '"') {
        inQuote = false;
      } else {
        if (c === '\n' || (c === '\r' && text[i + 1] !== '\n')) line += 1;
        cell += c;
      }
    } else if (c === '"' && cell.trim() === '') {
      inQuote = true;
      quoteStart = line;
      cell = '';
    } else if (c === delimiter) {
      endCell();
    } else if (c === '\r' || c === '\n') {
      if (c === '\r' && text[i + 1] === '\n') i += 1;
      endRow();
      line += 1;
      rowLine = line;
    } else {
      cell += c;
    }
  }
  if (inQuote) {
    const fail = new ApiFail(400, 'invalid_csv', `${quoteStart}行目の引用符（"）が閉じていません。表計算ソフトで開き直して、CSVで保存し直してください。`, null);
    // 本物のサーバーと同じく、行番号を欄（line）にも入れる。
    fail.line = quoteStart;
    throw fail;
  }
  if (cell !== '' || cells.length) endRow();
  return rows;
}

// ---- 書き出し ----

const FORMULA_START = /^[=+\-@\t\r]/;

/** 文字列の欄の先頭が = + - @ タブ CR なら、先頭に ' を足す（表計算ソフトで式として動かないように）。 */
export function guardCell(value) {
  if (typeof value === 'number') return String(value);
  const s = String(value ?? '');
  return FORMULA_START.test(s) ? `'${s}` : s;
}

function quote(s) {
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** 行の配列をCSVの文字にする（改行はCRLF、引用符はRFC 4180）。数値の欄はそのまま。 */
export function toCsv(rows) {
  return rows.map((r) => r.map((v) => quote(guardCell(v))).join(',')).join('\r\n') + '\r\n';
}

let sjisTable = null;

// Shift_JIS の逆引き表を、ブラウザと同じ TextDecoder から作る（依存を足さないため）。
function sjisMap() {
  if (sjisTable) return sjisTable;
  sjisTable = new Map();
  const dec = new TextDecoder('shift_jis');
  for (let b = 0xa1; b <= 0xdf; b += 1) sjisTable.set(dec.decode(Uint8Array.of(b)), [b]);
  const leads = [];
  for (let b = 0x81; b <= 0x9f; b += 1) leads.push(b);
  for (let b = 0xe0; b <= 0xfc; b += 1) leads.push(b);
  for (const lead of leads) {
    for (let trail = 0x40; trail <= 0xfc; trail += 1) {
      if (trail === 0x7f) continue;
      const ch = dec.decode(Uint8Array.of(lead, trail));
      if (ch.length === 1 && ch !== REPLACEMENT && !sjisTable.has(ch)) sjisTable.set(ch, [lead, trail]);
    }
  }
  return sjisTable;
}

/** 文字を Shift_JIS のバイト列にする。表せない文字は ? にして、その数を返す。 */
export function encodeSjis(text) {
  const map = sjisMap();
  const out = [];
  let lost = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0);
    if (code < 0x80) out.push(code);
    else if (map.has(ch)) out.push(...map.get(ch));
    else {
      out.push(0x3f);
      lost += 1;
    }
  }
  return { bytes: Buffer.from(out), lost };
}

/** CSVの応え（{ __raw }）を作る。encoding は utf8bom（既定）か sjis。 */
export function csvResponse(rows, filename, encoding = 'utf8bom') {
  const text = toCsv(rows);
  if (encoding === 'sjis') {
    const { bytes, lost } = encodeSjis(text);
    // 本物のサーバーと同じ形（Shift_JIS に無い文字を ? にした数）。
    const headers = lost ? { 'X-Kamoku-Warning': `unmappable=${lost}` } : {};
    return { __raw: bytes, contentType: 'text/csv; charset=Shift_JIS', filename, headers };
  }
  return { __raw: Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(text, 'utf8')]), contentType: 'text/csv; charset=utf-8', filename, headers: {} };
}
