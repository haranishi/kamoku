// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 書き出し（api-reports.md の「書き出し」）。CSVは改行CRLF、引用符はRFC 4180、文字列の欄の式の実行を防ぐ。
// ファイル名は Content-Disposition で付ける。一時の道具で、本物は kamoku-app が持つ。

import { ApiFail } from './compose.mjs';
import { csvResponse } from './csvio.mjs';
import { trialBalance, incomeStatement, balanceSheet, accountLedger, taxSummary } from './books.mjs';

const KIND_JA = { asset: '資産', liability: '負債', equity: '資本', revenue: '収益', expense: '費用' };

function encodingOf(q) {
  const e = q.encoding || 'utf8bom';
  if (e !== 'utf8bom' && e !== 'sjis') throw new ApiFail(400, 'bad_request', '文字コードは utf8bom か sjis にしてください。', 'encoding');
  return e;
}

function journalCsv(store, q) {
  const state = q.state || 'active';
  const rows = [['仕訳No', '版', '状態', '日付', '摘要', '取引先', '行', '貸借', '科目コード', '科目名', '金額', '税区分', '税額', '行メモ']];
  for (const s of store.all()) {
    const e = s.entry;
    if (state === 'active' && s.voided) continue;
    if (q.from && e.date < q.from) continue;
    if (q.to && e.date > q.to) continue;
    e.lines.forEach((l, i) => {
      rows.push([s.no, s.revision, s.voided ? '取消済み' : '有効', e.date, e.description, e.partner, i + 1, l.side === 'debit' ? '借方' : '貸方', l.account, store.nameOf(l.account), l.amount, l.tax_code || '', l.tax_amount, l.memo]);
    });
  }
  return rows;
}

function flatBs(section, label, depth = 0, out = []) {
  for (const r of section) {
    out.push([label, r.code, `${'　'.repeat(depth)}${r.name}`, r.amount]);
    if (r.children) flatBs(r.children, label, depth + 1, out);
  }
  return out;
}

const MAKERS = {
  'journal.csv': (store, q) => journalCsv(store, q),
  'trial-balance.csv': (store, q) => {
    const tb = trialBalance(store, q);
    return [['科目コード', '科目名', '分類', '期首', '借方', '貸方', '期末'], ...tb.rows.map((r) => [r.code, r.name, KIND_JA[r.kind], r.opening, r.debit, r.credit, r.closing]), ['', '合計', '', '', tb.total_debit, tb.total_credit, '']];
  },
  'income-statement.csv': (store, q) => {
    const is = incomeStatement(store, q);
    if (q.by === 'month') {
      const head = ['区分', '科目コード', '科目名', ...Array.from({ length: 12 }, (_, i) => `${i + 1}月`), '合計'];
      const body = (label, sec) => sec.rows.map((r) => [label, r.code, r.name, ...r.months, r.amount]);
      return [head, ...body('収益', is.revenue), ...body('費用', is.expense), ['', '', '当期純利益', ...is.months.map((m) => m.net_income), is.net_income]];
    }
    return [['区分', '科目コード', '科目名', '金額'], ...is.revenue.rows.map((r) => ['収益', r.code, r.name, r.amount]), ['収益', '', '収益の合計', is.revenue.total], ...is.expense.rows.map((r) => ['費用', r.code, r.name, r.amount]), ['費用', '', '費用の合計', is.expense.total], ['', '', '当期純利益', is.net_income]];
  },
  'balance-sheet.csv': (store, q, today) => {
    const bs = balanceSheet(store, q, today);
    return [['区分', '科目コード', '科目名', '金額'], ...flatBs(bs.assets.rows, '資産'), ['資産', '', '資産の合計', bs.assets.total], ...flatBs(bs.liabilities.rows, '負債'), ['負債', '', '負債の合計', bs.liabilities.total], ...flatBs(bs.equity.rows, '資本'), ['資本', '', '資本の合計', bs.equity.total]];
  },
  'ledger.csv': (store, q) => {
    const l = accountLedger(store, q);
    return [['日付', '仕訳No', '摘要', '相手科目', '借方', '貸方', '残高'], ['', '', '前期からの繰越', '', '', '', l.opening], ...l.rows.map((r) => [r.date, r.no, r.description, r.counter, r.debit, r.credit, r.balance])];
  },
  'tax-summary.csv': (store, q) => {
    const t = taxSummary(store, q);
    return [['税区分', '名前', '課税標準（税込）', '税額'], ...t.rows.map((r) => [r.tax_code, r.label, r.gross, r.tax]), ['', '売上税額', '', t.sales_tax], ['', '仕入税額', '', t.purchase_tax], ['', '見込み額（参考）', '', t.estimate]];
  },
};

/** GET /export/<name> */
export function exportFile(store, name, q, today) {
  if (name === 'all.json') {
    const body = { schema: 1, settings: store.db.settings, accounts: store.db.accounts, partners: store.db.partners, events: store.db.events };
    return { __raw: Buffer.from(JSON.stringify(body, null, 2), 'utf8'), contentType: 'application/json; charset=utf-8', filename: 'kamoku-all.json', headers: {} };
  }
  if (name === 'backup.db') throw new ApiFail(501, 'not_implemented', '模擬サーバーにはDBが無いため、バックアップは作れません。本物のサーバーで試してください。', null);
  const make = MAKERS[name];
  if (!make) throw new ApiFail(404, 'not_found', 'その書き出しはありません。', null);
  return csvResponse(make(store, q, today), name, encodingOf(q));
}
