// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 一覧の絞り込みと、ホーム用のまとめ（docs/spec/api-entries.md、api-reports.md、api-masters.md）。
// 集計に使うのは、取消を除いた最新の版の仕訳だけ。

const pad2 = (n) => String(n).padStart(2, '0');
const lastDay = (y, m) => new Date(Date.UTC(y, m, 0)).getUTCDate();

/** GET /entries の絞り込み。日付の新しい順、同じ日は番号の大きい順。state の既定は active（api-entries.md）。 */
export function listEntries(store, q) {
  const state = q.state || 'active';
  const needle = (q.q || '').trim();
  const digits = needle.replace(/[,，円¥]/g, '');
  let rows = store.all().filter((s) => {
    const e = s.entry;
    if (state === 'active' && s.voided) return false;
    if (state === 'voided' && !s.voided) return false;
    if (q.from && e.date < q.from) return false;
    if (q.to && e.date > q.to) return false;
    if (q.kind && e.kind !== q.kind) return false;
    if (q.source && e.meta.source !== q.source) return false;
    if (q.account && !e.lines.some((l) => l.account === q.account)) return false;
    if (q.wallet && !e.lines.some((l) => l.account === q.wallet)) return false;
    const total = e.lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0);
    if (q.min && total < Number(q.min)) return false;
    if (q.max && total > Number(q.max)) return false;
    if (needle) {
      const hay = [e.description, e.partner, ...e.lines.map((l) => `${l.memo} ${store.nameOf(l.account)}`)].join(' ');
      const amounts = [total, ...e.lines.map((l) => l.amount)].map(String);
      const hit = hay.includes(needle) || (/^\d+$/.test(digits) && amounts.some((a) => a.includes(digits)));
      if (!hit) return false;
    }
    return true;
  });
  rows.sort((a, b) => (a.entry.date === b.entry.date ? b.no - a.no : a.entry.date < b.entry.date ? 1 : -1));
  const sumAmount = rows.reduce((acc, s) => acc + s.entry.lines.filter((l) => l.side === 'debit').reduce((a, l) => a + l.amount, 0), 0);
  const limit = Math.min(Math.max(Number(q.limit) || 50, 1), 500);
  const offset = Math.max(Number(q.offset) || 0, 0);
  const total = rows.length;
  rows = rows.slice(offset, offset + limit);
  return { items: rows.map((s) => store.toRepr(s, false)), total, sum_amount: sumAmount };
}

function activeEntries(store, from, to) {
  return store
    .all()
    .filter((s) => !s.voided && (!from || s.entry.date >= from) && (!to || s.entry.date <= to))
    .map((s) => s.entry);
}

/** 売上・経費・利益。事業主貸と事業主借は、資産と負債なので入らない（AC-07.3）。 */
function profitLoss(store, from, to) {
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a.kind]));
  let revenue = 0;
  let expense = 0;
  for (const e of activeEntries(store, from, to)) {
    for (const l of e.lines) {
      const k = kinds.get(l.account);
      if (k === 'revenue') revenue += l.side === 'credit' ? l.amount : -l.amount;
      if (k === 'expense') expense += l.side === 'debit' ? l.amount : -l.amount;
    }
  }
  return { revenue, expense, profit: revenue - expense };
}

function monthBounds(ym) {
  const [y, m] = ym.split('-').map(Number);
  return { from: `${ym}-01`, to: `${ym}-${pad2(lastDay(y, m))}`, y, m };
}

function prevMonth(ym) {
  const [y, m] = ym.split('-').map(Number);
  return m === 1 ? `${y - 1}-12` : `${y}-${pad2(m - 1)}`;
}

/** GET /wallets。カードの残高は未払い額（正）。 */
export function walletBalances(store, asOf) {
  const wallets = store.db.accounts.filter((a) => a.wallet && a.enabled);
  const sums = new Map(wallets.map((w) => [w.code, 0]));
  for (const e of activeEntries(store, null, asOf)) {
    for (const l of e.lines) {
      if (!sums.has(l.account)) continue;
      sums.set(l.account, sums.get(l.account) + (l.side === 'debit' ? l.amount : -l.amount));
    }
  }
  let cashTotal = 0;
  let owedTotal = 0;
  const items = wallets.map((w) => {
    const debitMinusCredit = sums.get(w.code);
    const balance = w.wallet.type === 'card' ? -debitMinusCredit : debitMinusCredit;
    if (w.wallet.type === 'card') owedTotal += balance;
    else cashTotal += balance;
    return { code: w.code, name: w.name, type: w.wallet.type, balance };
  });
  return { items, cash_total: cashTotal, owed_total: owedTotal };
}

/** GET /reports/dashboard?month=YYYY-MM。 */
export function dashboard(store, month, today) {
  const { from, to, y } = monthBounds(month);
  const prev = monthBounds(prevMonth(month));
  const cur = profitLoss(store, from, to);
  const before = profitLoss(store, prev.from, prev.to);
  const ytd = profitLoss(store, `${y}-01-01`, to);
  const series = [];
  for (let m = 1; m <= 12; m += 1) {
    const b = monthBounds(`${y}-${pad2(m)}`);
    series.push({ month: `${y}-${pad2(m)}`, ...profitLoss(store, b.from, b.to) });
  }
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a]));
  const byAccount = new Map();
  for (const e of activeEntries(store, from, to)) {
    for (const l of e.lines) {
      const a = kinds.get(l.account);
      if (!a || a.kind !== 'expense') continue;
      byAccount.set(a.code, (byAccount.get(a.code) || 0) + (l.side === 'debit' ? l.amount : -l.amount));
    }
  }
  const topExpenses = [...byAccount.entries()]
    .filter(([, amount]) => amount !== 0)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
    .map(([code, amount]) => ({ account: code, name: kinds.get(code).name, amount }));
  const drafts = store.db.drafts.filter((d) => d.status === 'draft');
  const asOf = to < today ? to : today;
  const events = store.db.events;
  return {
    month,
    fiscal_year: y,
    next_actions: {
      inbox_draft: drafts.length,
      inbox_out_amount: drafts.filter((d) => d.direction === 'out').reduce((a, d) => a + d.amount, 0),
    },
    month_summary: { ...cur, prev: before },
    ytd,
    wallets: walletBalances(store, asOf),
    series,
    top_expenses: topExpenses,
    tax: taxEstimate(store, `${y}-01-01`, to, asOf),
    ledger: { events: events.length, head_hash: events.length ? events[events.length - 1].hash : '0'.repeat(64), verified: !store.db.broken },
  };
}

function taxEstimate(store, from, to, asOf) {
  if (store.db.settings.tax_mode !== 'taxable') return { applicable: false };
  let sales = 0;
  let purchase = 0;
  for (const e of activeEntries(store, from, to)) {
    for (const l of e.lines) {
      const signed = l.side === 'credit' ? l.tax_amount : -l.tax_amount;
      if (l.tax_code === 'S10' || l.tax_code === 'S8') sales += signed;
      if (l.tax_code === 'P10' || l.tax_code === 'P8') purchase -= signed;
    }
  }
  return { applicable: true, sales_tax: sales, purchase_tax: purchase, estimate: sales - purchase, as_of: asOf };
}
