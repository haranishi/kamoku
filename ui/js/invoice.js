// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 適格請求書発行事業者の登録番号（T + 13桁）。形式だけを確かめる（SC-09、SC-10）。
// 実在の確認と、チェックの数字の検査はしない（形式の検査はサーバーも必ず行う）。

/**
 * 打った文字を、送る形にそろえる。全角を半角に、小文字の t を T に、空白とハイフンを除く。
 * @returns {{ ok: true, value: string|null } | { ok: false, error: string }}
 */
export function normalizeInvoice(text) {
  const v = String(text ?? '').normalize('NFKC').replace(/[\s\-‐−ー]/g, '').toUpperCase();
  if (!v) return { ok: true, value: null };
  if (!/^T\d{13}$/.test(v)) return { ok: false, error: '登録番号は、T と13桁の数字で入れてください（例: T1234567890123）。' };
  return { ok: true, value: v };
}
