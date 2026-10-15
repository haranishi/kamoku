// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の「確定候補」の印（SC-03）。画面の中だけの集合で、台帳にも下書きにも何も書かない。
// 絞り込みやページを変えても印を保つため、行の表示とは別に、id ごとに金額と向きを覚えておく。

/** 確定は一度に2,000件まで（api-inbox.md。超えるとサーバーが何も始めずに 422 で断る）。 */
export const CONFIRM_LIMIT = 2000;

/** 番号の並びを、size 件ずつの組に分ける（送った順を保つ）。 */
export function chunkIds(ids, size = CONFIRM_LIMIT) {
  const out = [];
  for (let i = 0; i < ids.length; i += size) out.push(ids.slice(i, i + size));
  return out;
}

/**
 * 確定候補を、上限の件数ずつ順に送り、結果をまとめる。途中の組で送れなかったら、そこで止めて error に入れる
 * （投げずに返すのは、それまでの組で確定した分の印を、呼ぶ側が外せるようにするため）。
 * @param {number[]} ids
 * @param {(ids: number[]) => Promise<{ confirmed: any[], failed: any[] }>} send
 * @returns {Promise<{ confirmed: any[], failed: any[], error: Error|null }>}
 */
export async function confirmInChunks(ids, send, size = CONFIRM_LIMIT) {
  const confirmed = [];
  const failed = [];
  for (const part of chunkIds(ids, size)) {
    try {
      const res = await send(part);
      confirmed.push(...res.confirmed);
      failed.push(...res.failed);
    } catch (error) {
      return { confirmed, failed, error };
    }
  }
  return { confirmed, failed, error: null };
}

/**
 * 確定した仕訳の番号の文。「No.206〜No.225」。1件なら「No.212」だけ（「No.212〜No.212」と範囲の形になっていた。E4 第2回）。
 * @param {number[]} nos
 */
export function confirmedNos(nos) {
  const sorted = [...nos].sort((a, b) => a - b);
  if (!sorted.length) return '';
  const [first, last] = [sorted[0], sorted[sorted.length - 1]];
  return first === last ? `No.${first}` : `No.${first}〜No.${last}`;
}

// もう確定できない行（ほかで確定済み・見つからない）。印を外す（残すと、次の確定で毎回失敗する）。
const GONE = new Set(['already_confirmed', 'not_found']);

/**
 * 確定の結果を仕分ける: 印を外す番号（確定できた行と、もう確定できない行）、直せる失敗（行に理由を出す）、もう確定できない失敗。
 * @param {{ confirmed: Array<{ id: number }>, failed: Array<{ id: number, error?: { code?: string } }> }} res
 */
export function confirmOutcome(res) {
  const gone = res.failed.filter((f) => f.error && GONE.has(f.error.code));
  const fixable = res.failed.filter((f) => !gone.includes(f));
  return { unmark: [...res.confirmed.map((c) => c.id), ...gone.map((f) => f.id)], gone, fixable };
}

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
    /**
     * 未確認の全部の行の番号を受け取り、そこに無い印を外す（ほかで確定・無視された行は、もう確定できない）。
     * 絞り込みやページの外の行の印を消さないよう、全部の行が分かるときだけ呼ぶ。外した数を返す。
     */
    keepOnly(waiting) {
      let n = 0;
      for (const id of [...marks.keys()]) {
        if (!waiting.has(id)) {
          marks.delete(id);
          n += 1;
        }
      }
      return n;
    },
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
