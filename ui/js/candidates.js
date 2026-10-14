// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の「確定候補」の印（SC-03）。画面の中だけの集合で、台帳にも下書きにも何も書かない。
// 絞り込みやページを変えても印を保つため、行の表示とは別に、id ごとに金額と向きを覚えておく。

/**
 * 確定候補の集合を作る。
 * 例: const c = createCandidates(); c.toggle(draft); c.summary() → { count, inTotal, outTotal }
 */
export function createCandidates() {
  const marks = new Map();
  return {
    has: (id) => marks.has(id),
    /** 印を付ける（行の金額と向きを覚える）。 */
    add(draft) {
      marks.set(draft.id, { direction: draft.direction, amount: draft.amount });
    },
    remove(id) {
      marks.delete(id);
    },
    /** 付いていれば外し、無ければ付ける。付いた後の状態を返す。 */
    toggle(draft) {
      if (marks.has(draft.id)) {
        marks.delete(draft.id);
        return false;
      }
      marks.set(draft.id, { direction: draft.direction, amount: draft.amount });
      return true;
    },
    ids: () => [...marks.keys()],
    get size() {
      return marks.size;
    },
    clear: () => marks.clear(),
    /** 件数と、入金の合計・出金の合計（確定の前の確認と、下の固定バーに出す）。 */
    summary() {
      let inTotal = 0;
      let outTotal = 0;
      for (const m of marks.values()) {
        if (m.direction === 'in') inTotal += m.amount;
        else outTotal += m.amount;
      }
      return { count: marks.size, inTotal, outTotal };
    },
  };
}
