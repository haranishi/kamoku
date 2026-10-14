// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 金額と日付の入力を読む。どちらも失敗の理由を、そのまま画面に出せる文で返す。
// 金額の式は BigInt で計算する。途中の値が大きくなっても、桁が落ちて別の金額になることがないように。

/** 1明細の上限（data-model.md の決まり7）。 */
export const MAX_AMOUNT = 9999999999999;
const MAX_BIG = BigInt(MAX_AMOUNT);
const MAX_EXPR_LENGTH = 100;
const MAX_DEPTH = 20;

/**
 * 全角を半角に、いろいろなマイナスを「-」に、×÷を「* /」にそろえる。
 * 日本語入力のままだと「ー」（長音）や「－」が打たれるので、それも引き算とみなす。
 */
export function normalizeWidth(text) {
  return String(text ?? '')
    .normalize('NFKC')
    .replace(/[−‐‑‒–—―ー﹣]/g, '-')
    .replace(/×/g, '*')
    .replace(/÷/g, '/');
}

class ParseFailure extends Error {}

function tokenize(src) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (c >= '0' && c <= '9') {
      let j = i;
      while (j < src.length && src[j] >= '0' && src[j] <= '9') j += 1;
      tokens.push(BigInt(src.slice(i, j)));
      i = j;
    } else if ('+-*/()'.includes(c)) {
      tokens.push(c);
      i += 1;
    } else if (c === '.') {
      throw new ParseFailure('小数は使えません。円の整数で入れてください（例: 1650）。');
    } else {
      throw new ParseFailure(`使えない文字「${c}」があります。数字と + - * / ( ) だけで入れてください。`);
    }
  }
  return tokens;
}

function evaluate(tokens) {
  let i = 0;
  let depth = 0;
  let rounded = false;
  const syntax = () => new ParseFailure('式の形が正しくありません（例: 1000+650）。');

  function factor() {
    const t = tokens[i];
    if (t === '+' || t === '-') {
      i += 1;
      depth += 1;
      if (depth > MAX_DEPTH) throw syntax();
      const v = factor();
      depth -= 1;
      return t === '-' ? -v : v;
    }
    if (t === '(') {
      i += 1;
      depth += 1;
      if (depth > MAX_DEPTH) throw new ParseFailure('かっこが深すぎます。式を短くしてください。');
      const v = expr();
      if (tokens[i] !== ')') throw new ParseFailure('かっこが閉じていません。「)」を足してください。');
      i += 1;
      depth -= 1;
      return v;
    }
    if (typeof t === 'bigint') {
      i += 1;
      return t;
    }
    throw syntax();
  }

  function term() {
    let v = factor();
    while (tokens[i] === '*' || tokens[i] === '/') {
      const op = tokens[i];
      i += 1;
      const r = factor();
      if (op === '*') {
        v *= r;
      } else {
        if (r === 0n) throw new ParseFailure('0では割れません。式を確かめてください。');
        if (v % r !== 0n) rounded = true;
        v /= r;
      }
    }
    return v;
  }

  function expr() {
    let v = term();
    while (tokens[i] === '+' || tokens[i] === '-') {
      const op = tokens[i];
      i += 1;
      const r = term();
      v = op === '+' ? v + r : v - r;
    }
    return v;
  }

  const value = expr();
  if (i !== tokens.length) throw syntax();
  return { value, rounded };
}

/**
 * 金額の欄を読む。「1,650」「１６５０」「1000+650」「(1200+300)*2」など。AC-02.3。
 * 割り算は切り捨てで、そのときは rounded が真になる。
 * @param {string} text
 * @returns {{ ok: true, value: number, rounded: boolean, isExpr: boolean } | { ok: false, error: string, empty?: boolean }}
 */
export function parseAmountExpr(text) {
  const src = normalizeWidth(text).replace(/[,\s円¥\\]/g, '');
  if (src === '') return { ok: false, empty: true, error: '金額を入れてください。' };
  if (src.length > MAX_EXPR_LENGTH) {
    return { ok: false, error: `式が長すぎます。${MAX_EXPR_LENGTH}文字以内にしてください。` };
  }
  try {
    const tokens = tokenize(src);
    const { value, rounded } = evaluate(tokens);
    if (value < 1n) return { ok: false, error: '金額は1円以上にしてください。' };
    if (value > MAX_BIG) return { ok: false, error: '金額は9,999,999,999,999円以下にしてください。' };
    const isExpr = tokens.some((t) => typeof t !== 'bigint');
    return { ok: true, value: Number(value), rounded, isExpr };
  } catch (e) {
    if (e instanceof ParseFailure) return { ok: false, error: e.message };
    throw e;
  }
}

const pad2 = (n) => String(n).padStart(2, '0');

/** 実在する日なら真。 */
export function isRealDate(y, m, d) {
  if (!Number.isInteger(y) || !Number.isInteger(m) || !Number.isInteger(d)) return false;
  if (y < 1 || y > 9999 || m < 1 || m > 12 || d < 1) return false;
  return d <= daysInMonth(y, m);
}

/** その月の日数。 */
export function daysInMonth(y, m) {
  return new Date(Date.UTC(y, m, 0)).getUTCDate();
}

function toIso(y, m, d) {
  return `${String(y).padStart(4, '0')}-${pad2(m)}-${pad2(d)}`;
}

/** この端末の今日を YYYY-MM-DD で。 */
export function todayIso(now = new Date()) {
  return toIso(now.getFullYear(), now.getMonth() + 1, now.getDate());
}

/** YYYY-MM-DD に n 日足す（負なら引く）。 */
export function addDays(iso, n) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return iso;
  const t = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]) + n));
  return toIso(t.getUTCFullYear(), t.getUTCMonth() + 1, t.getUTCDate());
}

const DATE_HELP = '日付は「1005」「10/5」「2026-10-05」の形で入れてください。「t」で今日になります。';

/**
 * 日付の欄を読む。AC-02.4。
 * 受け付ける形: 1005 / 10/5 / 10-5 / 2026/10/5 / 2026-10-05 / 20261005 / t（今日）/ y（昨日）。全角も可。
 * @param {string} text
 * @param {{ today?: string, year?: number }} [opts] today は YYYY-MM-DD。year は年を省いたときの年。
 * @returns {{ ok: true, value: string } | { ok: false, error: string, empty?: boolean }}
 */
export function parseDateInput(text, opts = {}) {
  const today = opts.today || todayIso();
  const baseYear = opts.year || Number(today.slice(0, 4));
  const src = normalizeWidth(text).trim().toLowerCase().replace(/\s+/g, '');
  if (src === '') return { ok: false, empty: true, error: '日付を入れてください。' };
  if (src === 't' || src === '今日') return { ok: true, value: today };
  if (src === 'y' || src === '昨日') return { ok: true, value: addDays(today, -1) };

  let y;
  let mo;
  let d;
  let m;
  if ((m = /^(\d{4})[/\-.年](\d{1,2})[/\-.月](\d{1,2})日?$/.exec(src))) {
    [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  } else if ((m = /^(\d{4})(\d{2})(\d{2})$/.exec(src))) {
    [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  } else if ((m = /^(\d{1,2})[/\-.月](\d{1,2})日?$/.exec(src))) {
    [y, mo, d] = [baseYear, Number(m[1]), Number(m[2])];
  } else if ((m = /^(\d{2})(\d{2})$/.exec(src))) {
    [y, mo, d] = [baseYear, Number(m[1]), Number(m[2])];
  } else {
    return { ok: false, error: DATE_HELP };
  }
  if (!isRealDate(y, mo, d)) {
    return { ok: false, error: `${y}年${mo}月${d}日はありません。日付を確かめてください。` };
  }
  return { ok: true, value: toIso(y, mo, d) };
}

/** 「2026-10」の月の初日と末日。 */
export function monthRange(ym) {
  const [y, m] = ym.split('-').map(Number);
  return { from: toIso(y, m, 1), to: toIso(y, m, daysInMonth(y, m)) };
}

/** 「2026-10」を n か月ずらす。 */
export function shiftMonth(ym, n) {
  const [y, m] = ym.split('-').map(Number);
  const t = new Date(Date.UTC(y, m - 1 + n, 1));
  return `${t.getUTCFullYear()}-${pad2(t.getUTCMonth() + 1)}`;
}
