// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 科目を変えたときの、税区分と事業の割合の決め方。取引の登録と未確認の表で同じ決まりを使う。
// 決まり（api-inbox.md）: いまの値が「変える前の科目の既定値」と同じなら、新しい科目の既定値にする。
// 違えば、利用者が決めた値として残す。過去の入力から推測した値は使わない（AC-03.3、特許の確認が済むまで）。

/** 税区分の表示名（api-entries.md）。 */
export const TAX_LABELS = {
  S10: '課税売上10%',
  S8: '課税売上8%（軽減）',
  P10: '課税仕入10%',
  P8: '課税仕入8%（軽減）',
  P10X: '経過措置の仕入10%（免税事業者等から）',
  P8X: '経過措置の仕入8%（免税事業者等から）',
  NT: '非課税',
};

/** 種別ごとに出す税区分（空は「対象外」）。 */
export function taxOptions(kind) {
  const codes = kind === 'income' ? ['S10', 'S8', 'NT'] : ['P10', 'P8', 'P10X', 'P8X', 'NT'];
  return [{ value: '', label: '対象外' }, ...codes.map((c) => ({ value: c, label: TAX_LABELS[c] }))];
}

/** 経過措置の税区分なら真（日付の欄の下に、課税仕入れの日の案内を出す）。 */
export function isTransitional(code) {
  return code === 'P10X' || code === 'P8X';
}

/**
 * 科目を変えた後の税区分と割合。
 * 相手科目が口座（カードの引き落とし、現金の引き出しなど）なら、税区分は対象外・割合は100に固める（AC-02.8）。
 * @param {{ prev: object|null, next: object|null, tax: string|null, pct: number }} p
 *   prev と next は科目（api-masters.md の形）。tax と pct は、いまの欄の値。
 * @returns {{ tax: string|null, pct: number, locked: boolean }} locked が真なら、税区分と割合の欄を使えなくする
 */
export function nextDefaults({ prev, next, tax, pct }) {
  if (next && next.wallet) return { tax: null, pct: 100, locked: true };
  const prevTax = prev && !prev.wallet ? prev.default_tax ?? null : null;
  const prevPct = prev && !prev.wallet ? prev.default_pct ?? 100 : 100;
  const nextTax = next ? next.default_tax ?? null : null;
  const nextPct = next ? next.default_pct ?? 100 : 100;
  return {
    tax: (tax ?? null) === prevTax ? nextTax : tax ?? null,
    pct: pct === prevPct ? nextPct : pct,
    locked: false,
  };
}
