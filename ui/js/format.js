// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 金額は整数の円として受け取り、文字の操作だけで桁区切りを入れる。
// Intl に任せないのは、環境ごとに記号や空白が変わり、試験と画面の文字がずれるのを避けるため。

const WEEKDAYS = ['日', '月', '火', '水', '木', '金', '土'];

function groupDigits(digits) {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

/**
 * 金額を「11,000」の形にする。負は半角ハイフン「-1,200」。
 * @param {number} n 整数の円
 * @returns {string}
 */
export function formatYen(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '';
  const digits = groupDigits(String(Math.trunc(Math.abs(n))));
  return n < 0 ? `-${digits}` : digits;
}

/**
 * 符号つきの金額。正は「+1,650」、負は「-1,650」、0は「0」。
 * 入金と出金を色に頼らず見分けるために使う。
 */
export function formatSigned(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '';
  if (n > 0) return `+${formatYen(n)}`;
  return formatYen(n);
}

function parseIso(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(String(iso || ''));
  if (!m) return null;
  return { y: Number(m[1]), m: Number(m[2]), d: Number(m[3]) };
}

/** 曜日（「月」など）。 */
export function weekdayJa(iso) {
  const p = parseIso(iso);
  if (!p) return '';
  return WEEKDAYS[new Date(Date.UTC(p.y, p.m - 1, p.d)).getUTCDay()];
}

/** 「2026-10-05 (月)」。 */
export function formatDate(iso) {
  const p = parseIso(iso);
  if (!p) return String(iso || '');
  return `${iso} (${weekdayJa(iso)})`;
}

/** 「10/5」。狭い表で使う。 */
export function formatDateShort(iso) {
  const p = parseIso(iso);
  if (!p) return String(iso || '');
  return `${p.m}/${p.d}`;
}

/** 「2026年10月」。 */
export function formatMonth(ym) {
  const m = /^(\d{4})-(\d{2})$/.exec(String(ym || ''));
  if (!m) return String(ym || '');
  return `${m[1]}年${Number(m[2])}月`;
}

/** 「70%」。 */
export function formatPct(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '';
  return `${n}%`;
}

/** UNIX秒を、この端末の時刻で「2026-10-05 14:03」にする。 */
export function formatDateTime(sec) {
  if (typeof sec !== 'number' || !Number.isFinite(sec)) return '';
  const d = new Date(sec * 1000);
  const pad = (v) => String(v).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * グラフの目盛り用の短い金額。「5万」「1.5万」「120万」「1億」。
 * 表示だけに使い、計算には使わない。
 */
export function formatCompactYen(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '';
  const sign = n < 0 ? '-' : '';
  const a = Math.trunc(Math.abs(n));
  if (a < 10000) return sign + groupDigits(String(a));
  if (a < 100000000) {
    const man = Math.trunc(a / 10000);
    const tenth = Math.trunc((a % 10000) / 1000);
    return `${sign}${groupDigits(String(man))}${man < 10 && tenth ? `.${tenth}` : ''}万`;
  }
  const oku = Math.trunc(a / 100000000);
  const tenth = Math.trunc((a % 100000000) / 10000000);
  return `${sign}${groupDigits(String(oku))}${oku < 10 && tenth ? `.${tenth}` : ''}億`;
}

/**
 * 前の期間との差を、記号の向きと文で返す。「前月より 10,800円 多い」。
 * @param {number} current
 * @param {number} previous
 * @param {string} [prevLabel] 「前月」など
 * @returns {{ dir: 'up'|'down'|'same', text: string }}
 */
export function describeDiff(current, previous, prevLabel = '前月') {
  const diff = current - previous;
  if (diff === 0) return { dir: 'same', text: `${prevLabel}と同じ` };
  const amount = `${formatYen(Math.abs(diff))}円`;
  return diff > 0
    ? { dir: 'up', text: `${prevLabel}より ${amount} 多い` }
    : { dir: 'down', text: `${prevLabel}より ${amount} 少ない` };
}

/** ハッシュの先頭8桁。 */
export function shortHash(hash) {
  return String(hash || '').slice(0, 8);
}
