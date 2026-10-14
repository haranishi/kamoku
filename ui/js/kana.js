// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 候補を探すときの文字のそろえ方。「ツウシン」「つうしん」「ﾂｳｼﾝ」を同じものとして扱う。
// 漢字の読みは辞書を持たず、科目や取引先に登録した reading（ひらがな）で探す。

/**
 * 比べるための形にそろえる。NFKC（全角英数→半角、半角カナ→全角）、カタカナ→ひらがな、小文字、空白を除く。
 * @param {string} text
 * @returns {string}
 */
export function fold(text) {
  const nfkc = String(text ?? '').normalize('NFKC').toLowerCase();
  let out = '';
  for (const ch of nfkc) {
    const code = ch.codePointAt(0);
    // カタカナ（ァ〜ヶ）を、同じ並びのひらがなへ移す。
    if (code >= 0x30a1 && code <= 0x30f6) out += String.fromCodePoint(code - 0x60);
    else if (!/\s/.test(ch)) out += ch;
  }
  return out;
}

/**
 * 1つの候補が、打った文字にどれだけ合うか。前方一致は0、部分一致は1、合わなければ-1。
 * 名前（label）、読み（reading）、コード（value）のどれかに合えばよい。
 * @param {string} foldedQuery fold() 済みの文字
 * @param {{ value?: string, label?: string, reading?: string, keywords?: string }} item
 * @returns {number}
 */
export function matchRank(foldedQuery, item) {
  if (!foldedQuery) return 0;
  let best = -1;
  for (const field of [item.label, item.reading, item.value, item.keywords]) {
    if (!field) continue;
    const idx = fold(field).indexOf(foldedQuery);
    if (idx === 0) return 0;
    if (idx > 0) best = 1;
  }
  return best;
}

/**
 * 打った文字で候補を絞る。前方一致を上に、同じ順位の中では元の並び（呼び出し側が決めた優先）を保つ。
 * @template T
 * @param {T[]} items
 * @param {string} query
 * @returns {T[]}
 */
export function filterItems(items, query) {
  const q = fold(query);
  if (!q) return items.slice();
  const ranked = [];
  items.forEach((item, index) => {
    const rank = matchRank(q, item);
    if (rank >= 0) ranked.push({ item, rank, index });
  });
  ranked.sort((a, b) => a.rank - b.rank || a.index - b.index);
  return ranked.map((r) => r.item);
}

/**
 * 名前か読みが、打った文字とちょうど同じ候補。欄を離れたときの自動選択に使う。
 * @template T
 * @param {T[]} items
 * @param {string} text
 * @returns {T[]}
 */
export function exactMatches(items, text) {
  const q = fold(text);
  if (!q) return [];
  return items.filter((item) => fold(item.label) === q || (item.reading && fold(item.reading) === q));
}
