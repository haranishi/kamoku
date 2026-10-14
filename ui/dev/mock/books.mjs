// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票（api-reports.md）。集計は取消を除いた最新の版だけ。年またぎは「仮想の繰越」:
// 収益・費用・事業主貸・事業主借は各年0から始め、前の年までの分は元入金に入れる。一時の道具で、本物は kamoku-books。

import { ApiFail } from './compose.mjs';
import { chartOrder } from './accounts.mjs';

const DEBIT_NORMAL = new Set(['asset', 'expense']);
const CARRIED = new Set(['owner_draw', 'owner_inv']);
const TAX_ORDER = ['S10', 'S8', 'P10', 'P8', 'P10X', 'P8X', 'NT'];
const TAX_LABEL = {
  S10: '課税売上10%',
  S8: '課税売上8%（軽減）',
  P10: '課税仕入10%',
  P8: '課税仕入8%（軽減）',
  P10X: '免税事業者等からの課税仕入れ10%（経過措置）',
  P8X: '免税事業者等からの課税仕入れ8%（軽減・経過措置）',
  NT: '非課税',
};
// 経過措置の割合（kamoku-jp-tax の表と同じ。課税仕入れの日で引く）。
const TRANSITIONAL = [
  ['2023-10-01', '2026-09-30', 80],
  ['2026-10-01', '2028-09-30', 70],
  ['2028-10-01', '2030-09-30', 50],
  ['2030-10-01', '2031-09-30', 30],
];
export const TABLE_CHECKED_ON = '2026-10-14';

export const isDate = (s) => typeof s === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(s) && !Number.isNaN(Date.parse(`${s}T00:00:00Z`)) && new Date(`${s}T00:00:00Z`).toISOString().slice(0, 10) === s;
const sign = (kind) => (DEBIT_NORMAL.has(kind) ? 1 : -1);

/** 問い合わせの期間（from と to、または year）を読み、1つの暦年に収まることを確かめる（AC-08.8）。 */
export function period(q) {
  let { from, to } = q;
  if (q.year && /^\d{4}$/.test(q.year)) {
    from = from || `${q.year}-01-01`;
    to = to || `${q.year}-12-31`;
  }
  if (!isDate(from) || !isDate(to)) throw new ApiFail(400, 'bad_request', '期間は from と to（YYYY-MM-DD）か、year で指定してください。', 'from');
  if (from > to) throw new ApiFail(400, 'bad_request', '期間の始めが、終わりより後になっています。', 'from');
  if (from.slice(0, 4) !== to.slice(0, 4)) {
    throw new ApiFail(422, 'period_spans_years', '帳票の期間は、1つの暦年（1月1日〜12月31日）の中にしてください。年をまたぐときは、年ごとに分けて見てください。', null);
  }
  return { from, to, year: Number(from.slice(0, 4)) };
}

function activeEntries(store) {
  return store.all().filter((s) => !s.voided);
}

// 期間の初日と同じ日付の期首残高の仕訳は、動きでなく期首に入れる（本物のサーバーと同じ。api-reports.md の年またぎの決まり）。
const openingOnFirstDay = (e, from) => e.date === from && e.meta && e.meta.kind === 'opening';

function chart(store) {
  return [...store.db.accounts].sort(chartOrder);
}

/**
 * 期首（from の前日まで）の借方−貸方と、期間内の借方・貸方の動き。前の年までの収益・費用・事業主貸・事業主借は元入金へ。
 * @returns {Map<string, { pre: number, debit: number, credit: number }>}
 */
function carried(store, year, from, to) {
  const yearStart = `${year}-01-01`;
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a.kind]));
  const acc = new Map(store.db.accounts.map((a) => [a.code, { pre: 0, debit: 0, credit: 0 }]));
  for (const s of activeEntries(store)) {
    const e = s.entry;
    if (e.date > to) continue;
    for (const l of e.lines) {
      const kind = kinds.get(l.account);
      if (!kind) continue;
      const dc = l.side === 'debit' ? l.amount : -l.amount;
      if (e.date < yearStart) {
        const toCapital = kind === 'revenue' || kind === 'expense' || CARRIED.has(l.account);
        acc.get(toCapital ? 'capital' : l.account).pre += dc;
      } else if (e.date < from || openingOnFirstDay(e, from)) {
        acc.get(l.account).pre += dc;
      } else if (l.side === 'debit') acc.get(l.account).debit += l.amount;
      else acc.get(l.account).credit += l.amount;
    }
  }
  return acc;
}

/** GET /reports/trial-balance */
export function trialBalance(store, q) {
  const p = period(q);
  const book = carried(store, p.year, p.from, p.to);
  const rows = [];
  let totalDebit = 0;
  let totalCredit = 0;
  let debitSide = 0;
  let creditSide = 0;
  for (const a of chart(store)) {
    const b = book.get(a.code);
    const opening = sign(a.kind) * b.pre;
    const closing = opening + sign(a.kind) * (b.debit - b.credit);
    totalDebit += b.debit;
    totalCredit += b.credit;
    if (DEBIT_NORMAL.has(a.kind)) debitSide += closing;
    else creditSide += closing;
    if (opening || b.debit || b.credit || closing) rows.push({ code: a.code, name: a.name, kind: a.kind, opening, debit: b.debit, credit: b.credit, closing });
  }
  return { rows, total_debit: totalDebit, total_credit: totalCredit, balanced: totalDebit === totalCredit && debitSide === creditSide };
}

/** GET /reports/income-statement?by=total|month */
export function incomeStatement(store, q) {
  const p = period(q);
  const byMonth = q.by === 'month';
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a]));
  const sums = new Map();
  for (const s of activeEntries(store)) {
    const e = s.entry;
    if (e.date < p.from || e.date > p.to) continue;
    const m = Number(e.date.slice(5, 7)) - 1;
    for (const l of e.lines) {
      const a = kinds.get(l.account);
      if (!a || (a.kind !== 'revenue' && a.kind !== 'expense')) continue;
      const v = (l.side === 'debit' ? l.amount : -l.amount) * sign(a.kind);
      if (!sums.has(a.code)) sums.set(a.code, Array(12).fill(0));
      sums.get(a.code)[m] += v;
    }
  }
  const section = (kind) => {
    const rows = chart(store)
      .filter((a) => a.kind === kind && sums.has(a.code))
      .map((a) => {
        const months = sums.get(a.code);
        const row = { code: a.code, name: a.name, amount: months.reduce((x, y) => x + y, 0) };
        if (byMonth) row.months = months;
        return row;
      })
      .filter((r) => r.amount || (r.months && r.months.some(Boolean)));
    return { rows, total: rows.reduce((x, r) => x + r.amount, 0) };
  };
  const revenue = section('revenue');
  const expense = section('expense');
  const out = { revenue, expense, net_income: revenue.total - expense.total };
  if (byMonth) {
    out.months = Array.from({ length: 12 }, (_, i) => {
      const rv = revenue.rows.reduce((x, r) => x + r.months[i], 0);
      const ex = expense.rows.reduce((x, r) => x + r.months[i], 0);
      return { month: `${p.year}-${String(i + 1).padStart(2, '0')}`, revenue: rv, expense: ex, net_income: rv - ex };
    });
  }
  return out;
}

function tree(accounts, balanceOf) {
  const children = new Map();
  for (const a of accounts) {
    if (!a.parent) continue;
    if (!children.has(a.parent)) children.set(a.parent, []);
    children.get(a.parent).push(a);
  }
  const codes = new Set(accounts.map((a) => a.code));
  const build = (a) => {
    const kids = (children.get(a.code) || []).map(build).filter(Boolean);
    const own = balanceOf(a);
    const amount = own + kids.reduce((x, k) => x + k.amount, 0);
    if (!amount && !kids.length) return null;
    const row = { code: a.code, name: a.name, amount, own };
    if (kids.length) row.children = kids;
    return row;
  };
  const rows = accounts.filter((a) => !a.parent || !codes.has(a.parent)).map(build).filter(Boolean);
  return { rows, total: rows.reduce((x, r) => x + r.amount, 0) };
}

/** GET /reports/balance-sheet?as_of= */
export function balanceSheet(store, q, today) {
  const asOf = q.as_of || today;
  if (!isDate(asOf)) throw new ApiFail(400, 'bad_request', '日付は YYYY-MM-DD の形にしてください。', 'as_of');
  const year = Number(asOf.slice(0, 4));
  // 前の年までは繰越（pre）、その年の1月1日から as_of までは動き。足すと as_of 時点の借方−貸方になる。
  const book = carried(store, year, `${year}-01-01`, asOf);
  const dc = (code) => {
    const b = book.get(code);
    return b.pre + b.debit - b.credit;
  };
  let net = 0;
  for (const a of store.db.accounts) {
    if (a.kind === 'revenue' || a.kind === 'expense') net -= dc(a.code);
  }
  const all = chart(store);
  const of = (kind) => all.filter((a) => a.kind === kind);
  const assets = tree(of('asset'), (a) => dc(a.code));
  const liabilities = tree(of('liability'), (a) => -dc(a.code));
  const eq = tree(of('equity'), (a) => -dc(a.code));
  eq.rows.push({ code: 'net_income_current', name: '当期純利益', amount: net, own: net });
  eq.total += net;
  return { as_of: asOf, assets, liabilities, equity: eq, net_income_current: net, balanced: assets.total === liabilities.total + eq.total };
}

/** GET /reports/ledger?account&from&to。1明細が1行。相手は反対側の科目（2つ以上なら「諸口」）。 */
export function accountLedger(store, q) {
  const p = period(q);
  const a = store.db.accounts.find((x) => x.code === q.account);
  if (!a) throw new ApiFail(422, 'unknown_account', '科目を選んでください。', 'account');
  const book = carried(store, p.year, p.from, p.to);
  const opening = sign(a.kind) * book.get(a.code).pre;
  let balance = opening;
  const rows = [];
  const entries = activeEntries(store)
    .filter((s) => s.entry.date >= p.from && s.entry.date <= p.to && !openingOnFirstDay(s.entry, p.from))
    .sort((x, y) => (x.entry.date === y.entry.date ? x.no - y.no : x.entry.date < y.entry.date ? -1 : 1));
  for (const s of entries) {
    for (const l of s.entry.lines) {
      if (l.account !== a.code) continue;
      const others = [...new Set(s.entry.lines.filter((o) => o.side !== l.side).map((o) => store.nameOf(o.account)))];
      const debit = l.side === 'debit' ? l.amount : 0;
      const credit = l.side === 'credit' ? l.amount : 0;
      balance += sign(a.kind) * (debit - credit);
      rows.push({ date: s.entry.date, no: s.no, entry_id: s.id, description: s.entry.description, counter: others.length > 1 ? '諸口' : others[0] || '', debit, credit, balance });
    }
  }
  return { account: a.code, opening, rows, closing: balance };
}

function transitionalPercent(date) {
  const r = TRANSITIONAL.find(([from, to]) => from <= date && date <= to);
  return r ? r[2] : 0;
}

/** GET /reports/tax-summary。国税と地方消費税の合計での参考値。 */
export function taxSummary(store, q) {
  const p = period(q);
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a.kind]));
  const rows = new Map();
  const trans = new Map();
  let sales = 0;
  let purchase = 0;
  for (const s of activeEntries(store)) {
    const e = s.entry;
    if (e.date < p.from || e.date > p.to) continue;
    for (const l of e.lines) {
      if (!l.tax_code) continue;
      const salesSide = l.tax_code.startsWith('S') || (l.tax_code === 'NT' && kinds.get(l.account) === 'revenue');
      const toward = salesSide ? 'credit' : 'debit';
      const gross = l.side === toward ? l.amount : -l.amount;
      const tax = l.tax_code === 'NT' ? 0 : l.side === toward ? l.tax_amount : -l.tax_amount;
      const r = rows.get(l.tax_code) || { tax_code: l.tax_code, label: TAX_LABEL[l.tax_code], gross: 0, tax: 0 };
      r.gross += gross;
      r.tax += tax;
      rows.set(l.tax_code, r);
      if (l.tax_code === 'S10' || l.tax_code === 'S8') sales += tax;
      if (l.tax_code === 'P10' || l.tax_code === 'P8') purchase += tax;
      if (l.tax_code === 'P10X' || l.tax_code === 'P8X') {
        const pct = transitionalPercent(e.date);
        const deductible = Math.floor((tax * pct) / 100);
        purchase += deductible;
        const t = trans.get(pct) || { percent: pct, gross: 0, tax: 0, deductible: 0 };
        t.gross += gross;
        t.tax += tax;
        t.deductible += deductible;
        trans.set(pct, t);
      }
    }
  }
  return {
    rows: TAX_ORDER.filter((c) => rows.has(c)).map((c) => rows.get(c)),
    sales_tax: sales,
    purchase_tax: purchase,
    estimate: sales - purchase,
    transitional: [...trans.values()].sort((x, y) => y.percent - x.percent),
    disclaimer: `参考値です。簡易課税と特例、経過措置の適用条件（請求書等と帳簿の保存、一の仕入先ごとの上限）は反映していません。課税の区分は現在の設定を使っています。経過措置の表の最終確認日は${TABLE_CHECKED_ON}です。`,
  };
}
