// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の期間（SC-06）。選び方は「年」「月」「自由な期間」、貸借対照表は日付を1つ。
// 期間はアドレスの ? に持つ（year=2026 / month=2026-10 / from=…&to=… / as_of=…）。
// 帳票は1つの暦年に収まる期間だけを受け付ける（AC-08.8）ので、またぐ選び方は選ぶ時点で止める。

import { monthRange, shiftMonth, isRealDate } from './input-parse.js';

const YEAR = /^\d{4}$/;
const MONTH = /^(\d{4})-(0[1-9]|1[0-2])$/;
const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/** YYYY-MM-DD として実在する日なら真。 */
export function isIsoDate(s) {
  const m = DATE.exec(String(s || ''));
  return Boolean(m) && isRealDate(Number(m[1]), Number(m[2]), Number(m[3]));
}

/**
 * 自由な期間を確かめる。良ければ空の文字、だめなら理由（画面にそのまま出す文）。
 * @param {string|null} from
 * @param {string|null} to
 */
export function checkRange(from, to) {
  if (!isIsoDate(from) || !isIsoDate(to)) return '期間の始めと終わりを、日付で入れてください。';
  if (from.slice(0, 4) !== to.slice(0, 4)) return `期間は同じ年の中で選んでください（いまは${from.slice(0, 4)}年と${to.slice(0, 4)}年をまたいでいます）。帳票は1年ずつ作ります。`;
  if (from > to) return '期間の始めが、終わりより後になっています。';
  return '';
}

/**
 * アドレスの条件から期間を読む。読めない値は既定（今年、または今日）にする。
 * @param {Record<string, string>} query
 * @param {{ today: string, dateMode?: boolean, yearOnly?: boolean }} opts
 *   dateMode: 貸借対照表（日付を1つ）。yearOnly: 月次推移（年だけ）。
 * @returns {{ mode: 'year'|'month'|'range'|'date', year: number, month?: string, from?: string, to?: string, asOf?: string }}
 */
export function readPeriod(query, { today, dateMode = false, yearOnly = false }) {
  const thisYear = Number(today.slice(0, 4));
  if (dateMode) {
    const asOf = isIsoDate(query.as_of) ? query.as_of : today;
    return { mode: 'date', year: Number(asOf.slice(0, 4)), asOf };
  }
  if (!yearOnly && MONTH.test(query.month || '')) {
    const r = monthRange(query.month);
    return { mode: 'month', year: Number(query.month.slice(0, 4)), month: query.month, from: r.from, to: r.to };
  }
  if (!yearOnly && query.from && query.to && !checkRange(query.from, query.to)) {
    return { mode: 'range', year: Number(query.from.slice(0, 4)), from: query.from, to: query.to };
  }
  const year = YEAR.test(query.year || '') ? Number(query.year) : thisYear;
  return { mode: 'year', year, from: `${year}-01-01`, to: `${year}-12-31` };
}

/** サーバーへの問い合わせの期間。 */
export function periodApi(p) {
  return p.mode === 'date' ? { as_of: p.asOf } : { from: p.from, to: p.to };
}

/** アドレスに書く期間（短い形）。 */
export function periodQuery(p) {
  if (p.mode === 'date') return { as_of: p.asOf };
  if (p.mode === 'month') return { month: p.month };
  if (p.mode === 'range') return { from: p.from, to: p.to };
  return { year: String(p.year) };
}

/** 期間の見出し。 */
export function periodLabel(p) {
  if (p.mode === 'date') return `${p.asOf} 時点`;
  if (p.mode === 'month') return `${p.year}年${Number(p.month.slice(5, 7))}月`;
  if (p.mode === 'range') return `${p.from} 〜 ${p.to}`;
  return `${p.year}年`;
}

/** 前・次へずらす（年なら1年、月なら1か月、日付なら1日ではなく1か月の末日へ）。自由な期間はずらさない。 */
export function shiftPeriod(p, n) {
  if (p.mode === 'year') {
    const year = p.year + n;
    return { mode: 'year', year, from: `${year}-01-01`, to: `${year}-12-31` };
  }
  if (p.mode === 'month') {
    const month = shiftMonth(p.month, n);
    const r = monthRange(month);
    return { mode: 'month', year: Number(month.slice(0, 4)), month, from: r.from, to: r.to };
  }
  if (p.mode === 'date') {
    const month = shiftMonth(p.asOf.slice(0, 7), n);
    const asOf = monthRange(month).to;
    return { mode: 'date', year: Number(asOf.slice(0, 4)), asOf };
  }
  return p;
}

/**
 * タブを移るときに、期間を引き継ぐ。年・月・自由な期間と、日付（貸借対照表）の間で読み替える。
 * @param {object} p いまの期間
 * @param {{ today: string, dateMode?: boolean, yearOnly?: boolean }} to 移る先
 */
export function carryPeriod(p, { today, dateMode = false, yearOnly = false }) {
  if (dateMode) {
    if (p.mode === 'date') return p;
    const end = p.to;
    const asOf = end > today && p.from <= today ? today : end;
    return { mode: 'date', year: Number(asOf.slice(0, 4)), asOf };
  }
  if (p.mode === 'date' || yearOnly) {
    const year = p.year;
    return { mode: 'year', year, from: `${year}-01-01`, to: `${year}-12-31` };
  }
  return p;
}
