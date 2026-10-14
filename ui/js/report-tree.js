// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票の表の並び（SC-06）: 親の科目の下に子をインデントし、親の行に合計を出す。
// サーバーの試算表と損益計算書は科目ごとに平らな行で返る（親子を足さない）ので、科目の親子から木を組む。
// 親に直接の記帳があれば「直接の記帳」の子の行にして、親の行が子の行の合計と一致するように見せる。
// 区分の合計はサーバーの合計を使い、ここで足した親の行を合計に数えない（二重に数えないため）。

/** 直接の記帳の行の名前。 */
export const DIRECT_LABEL = 'この科目への直接の記帳';

const KIND_ORDER = { asset: 0, liability: 1, equity: 2, revenue: 3, expense: 4 };

/** 科目の並び（分類、sort、コードの順。spec-issues の R1-12）。sort は分類をまたいで重なるので、分類を先に比べる。 */
export function chartCompare(a, b) {
  return (KIND_ORDER[a.kind] ?? 9) - (KIND_ORDER[b.kind] ?? 9) || (a.sort ?? 0) - (b.sort ?? 0) || (a.code < b.code ? -1 : a.code > b.code ? 1 : 0);
}

function zero(keys) {
  return Object.fromEntries(keys.map((k) => [k, 0]));
}

function addInto(target, row, keys) {
  for (const k of keys) {
    const v = row[k];
    if (Array.isArray(v)) target[k] = (Array.isArray(target[k]) ? target[k] : v.map(() => 0)).map((x, i) => x + v[i]);
    else target[k] = (target[k] || 0) + (v || 0);
  }
}

/**
 * 平らな行（科目ごと）を、科目の親子で木にして、表に出す順の行の配列にする。
 * @param {Array<object>} rows サーバーの行（code と、keys の数値の列を持つ）
 * @param {Array<{ code: string, name: string, parent: string|null }>} accounts 科目（親を引くため）
 * @param {string[]} keys 足す数値の列（'amount'、または 'opening' 'debit' 'credit' 'closing'。配列の列（months）も足せる）
 * @returns {Array<object>} 元の行に depth（0から）、parent（親の行なら真）、direct（直接の記帳の行なら真）を足したもの
 */
export function treeRows(rows, accounts, keys) {
  const byCode = new Map(accounts.map((a) => [a.code, a]));
  const rowOf = new Map(rows.map((r) => [r.code, r]));
  const kids = new Map();
  const topCodes = [];
  // 親が無い行、または親の科目が分からない行は、いちばん上に置く。親が行に無ければ、合計だけの親の行を作る。
  const ensure = (code) => {
    const a = byCode.get(code);
    const parent = a && a.parent && byCode.has(a.parent) ? a.parent : null;
    if (parent) {
      if (!kids.has(parent)) {
        kids.set(parent, []);
        ensure(parent);
      }
      if (!kids.get(parent).includes(code)) kids.get(parent).push(code);
    } else if (!topCodes.includes(code)) topCodes.push(code);
  };
  rows.forEach((r) => ensure(r.code));
  // 合計だけの親の行は、最初の子が出た位置に入るので、科目の並びに並べ直す。
  const order = (x, y) => {
    const a = byCode.get(x);
    const b = byCode.get(y);
    if (!a || !b) return 0;
    return chartCompare(a, b);
  };
  topCodes.sort(order);
  for (const list of kids.values()) list.sort(order);
  const out = [];
  const visit = (code, depth) => {
    const own = rowOf.get(code);
    const children = kids.get(code) || [];
    if (!children.length) {
      out.push({ ...own, depth, parent: false, direct: false });
      return { ...own };
    }
    const a = byCode.get(code);
    const head = { ...(own || {}), ...zero(keys), code, name: own ? own.name : a.name, depth, parent: true, direct: false };
    out.push(head);
    const hasOwn = own && keys.some((k) => (Array.isArray(own[k]) ? own[k].some(Boolean) : own[k]));
    if (hasOwn) {
      out.push({ ...own, name: DIRECT_LABEL, depth: depth + 1, parent: false, direct: true });
      addInto(head, own, keys);
    }
    for (const c of children) addInto(head, visit(c, depth + 1), keys);
    return head;
  };
  topCodes.forEach((c) => visit(c, 0));
  return out;
}

/**
 * 貸借対照表の行（親が children を持つ木）を、表に出す順の行にする。親の amount はサーバーの合計をそのまま使う。
 * @param {Array<{ code: string, name: string, amount: number, own: number, children?: any[] }>} rows
 */
export function flattenBsRows(rows, depth = 0, out = []) {
  for (const r of rows) {
    const children = r.children || [];
    out.push({ code: r.code, name: r.name, amount: r.amount, depth, parent: children.length > 0, direct: false });
    if (children.length && r.own) out.push({ code: r.code, name: DIRECT_LABEL, amount: r.own, depth: depth + 1, parent: false, direct: true });
    flattenBsRows(children, depth + 1, out);
  }
  return out;
}
