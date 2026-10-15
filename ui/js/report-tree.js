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
    const row = { code: r.code, name: r.name, amount: r.amount, depth, parent: children.length > 0, direct: false };
    if (r.sub) row.sub = r.sub;
    out.push(row);
    if (children.length && r.own) out.push({ code: r.code, name: DIRECT_LABEL, amount: r.own, depth: depth + 1, parent: false, direct: true });
    flattenBsRows(children, depth + 1, out);
  }
  return out;
}

/**
 * 平らな行（treeRows の結果）で、子が1つだけの親（直接の記帳の行が無く、子が末端）を1行に畳む。
 * 試算表・損益計算書で「現金／現金」と同じ数字が2回並ばないようにするため（貸借対照表の collapseSingleChildren と同じ決まり）。
 * 親の行の数字は子の合計なので、そのまま使う。子の名前が親と違えば sub に残す（孫まで1本道なら、途中の名前もつなぐ）。
 * @param {Array<{ name: string, depth: number, parent: boolean, direct: boolean, sub?: string }>} rows
 */
export function collapseSingleRows(rows) {
  let cur = rows;
  // 深い所から畳むと、上の親も1本道になることがあるので、変わらなくなるまで繰り返す（深さの数だけ）。
  for (;;) {
    const out = [];
    for (let k = 0; k < cur.length; k += 1) {
      const r = cur[k];
      let end = k + 1;
      while (end < cur.length && cur[end].depth > r.depth) end += 1;
      const kids = cur.slice(k + 1, end).filter((x) => x.depth === r.depth + 1);
      const only = r.parent && kids.length === 1 && end === k + 2 && !kids[0].parent && !kids[0].direct ? kids[0] : null;
      if (!only) {
        out.push(r);
        continue;
      }
      const names = [only.name, only.sub].filter((n) => n && n !== r.name);
      out.push({ ...r, parent: false, sub: names.length ? names.join('・') : r.sub });
      k += 1;
    }
    if (out.length === cur.length) return out;
    cur = out;
  }
}

/**
 * 月別の表に出す月（列）と、その月に科目の金額があるか。
 * 今月（until）より後の月も、科目の金額があれば出す。右端の「合計」はサーバーの年間の値なので、
 * 金額のある月を隠すと、列を足しても合計と合わなくなるため。
 * 「金額がある」は、科目の行の months[i] が1つでも0でないこと。振替だけの月は、収益と費用の合計が0でも、
 * 科目の金額はある（費用A +10,000 と 費用B −10,000）。
 * @param {Array<{ month: string }>} series 月ごとの行（その年の12か月）
 * @param {Array<{ months: number[] }>} rows 科目の行（収益と費用の全部）
 * @param {string} until YYYY-MM（今月。これより後は、金額のある月だけ）
 * @returns {Array<{ i: number, month: string, hasAmounts: boolean }>}
 */
export function monthColumns(series, rows, until) {
  return series
    .map((r, i) => ({ i, month: r.month, hasAmounts: rows.some((x) => Boolean(x.months && x.months[i])) }))
    .filter((c) => c.month <= until || c.hasAmounts);
}

/**
 * 月の並び（1〜12の数、小さい順）を短い言葉にする。2か月だけなら「11月と12月」。3か月以上続く所は「4月〜12月」に
 * まとめ、ほかは「・」で並べる（「1月〜10月・12月」）。
 * @param {number[]} months
 */
export function monthList(months) {
  if (months.length === 2) return `${months[0]}月と${months[1]}月`;
  const runs = [];
  for (const m of months) {
    const last = runs[runs.length - 1];
    if (last && m === last[last.length - 1] + 1) last.push(m);
    else runs.push([m]);
  }
  return runs.flatMap((r) => (r.length >= 3 ? [`${r[0]}月〜${r[r.length - 1]}月`] : r.map((m) => `${m}月`))).join('・');
}

/**
 * 月次推移の表に出ない月の一言（表の直前に出す。グラフは12月まであるのに、表が今月で終わる理由が画面に無かった）。
 * 出ない月が無ければ空の文字。今年で、今月より後の月を1つも出していないときだけ「表は今月までです。」を付ける
 * （金額のある後の月を出しているときは、表の題の「今月より後の月は、表だけに出ます」と食い違うため）。
 * @param {Array<{ month: string }>} series その年の12か月
 * @param {Array<{ i: number, month: string }>} cols monthColumns の結果（表に出す月）
 * @param {string} thisMonth 今月（YYYY-MM）
 */
export function hiddenMonthsNote(series, cols, thisMonth) {
  const shown = new Set(cols.map((c) => c.i));
  const hidden = series.flatMap((r, i) => (shown.has(i) ? [] : [Number(r.month.slice(5, 7))]));
  if (!hidden.length) return '';
  const thisYear = series.length > 0 && series[0].month.slice(0, 4) === thisMonth.slice(0, 4);
  const future = cols.some((c) => c.month > thisMonth);
  const head = thisYear && !future ? '表は今月までです。' : '';
  return `${head}${monthList(hidden)}は、その月になるか、記録ができたら出ます。`;
}

/**
 * 子が1つだけの親（自分への直接の記帳が無く、子が末端）を、1行に畳む。同じ金額が2回並ばないようにするため
 * （「現金／現金」「未払金／サンプルカード」）。子の名前が親と違えば、sub に残す（どの口座の残高か分かるように）。
 * 元の木は変えない。
 * @param {Array<{ code: string, name: string, amount: number, own?: number, children?: any[] }>} rows
 */
export function collapseSingleChildren(rows) {
  return rows.map((r) => {
    const children = (r.children || []).map((c) => collapseSingleChildren([c])[0]);
    const only = children.length === 1 ? children[0] : null;
    if (only && !r.own && !(only.children || []).length) {
      const merged = { ...r, children: [] };
      // 孫まで1本道なら、途中の名前も残す（親と同じ名前は省く）。
      const names = [only.name, only.sub].filter((n) => n && n !== r.name);
      if (names.length) merged.sub = names.join('・');
      return merged;
    }
    return { ...r, children };
  });
}
