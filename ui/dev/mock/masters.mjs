// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 科目・口座・取引先の追加と変更、期首残高（api-masters.md）。一時の道具で、本物は kamoku-app と kamoku-store。

import { ApiFail, TAX_CODES } from './compose.mjs';
import { PROTECTED_CODES, REQUIRED_CODES, WALLET_PARENT, walletAccount } from './accounts.mjs';
import { isDate } from './books.mjs';

const KINDS = ['asset', 'liability', 'equity', 'revenue', 'expense'];
const WALLET_TYPES = ['cash', 'bank', 'card', 'ewallet'];
const INVOICE = /^T\d{13}$/;

function name(v, field = 'name') {
  const s = String(v ?? '').trim();
  if (!s) throw new ApiFail(400, 'bad_request', '名前を入れてください。', field);
  return s;
}

function tax(v) {
  if (v === null || v === undefined || v === '') return null;
  if (!TAX_CODES.includes(v)) throw new ApiFail(422, 'bad_tax_code', `税区分「${v}」は使えません。一覧から選んでください。`, 'default_tax');
  return v;
}

function pct(v) {
  if (v === undefined || v === null) return 100;
  if (!Number.isInteger(v) || v < 0 || v > 100) throw new ApiFail(400, 'bad_request', '既定の事業割合は0から100の整数にしてください。', 'default_pct');
  return v;
}

function nextCode(store, prefix) {
  const max = Math.max(0, ...store.db.accounts.map((a) => (a.code.startsWith(prefix) ? Number(a.code.slice(1)) : 0)).filter(Number.isInteger));
  return `${prefix}${String(max + 1).padStart(2, '0')}`;
}

/** POST /accounts。口座なら w01 から、科目なら x01 から順にコードを付ける。 */
export function addAccount(store, body) {
  const b = body || {};
  const n = name(b.name);
  // 科目と口座の名前は重なってよい（標準の科目「現金」の子に、口座「現金」を置く）。重複を断るのは取引先だけ（api-masters.md）。
  if (b.wallet) {
    if (!WALLET_TYPES.includes(b.wallet.type)) throw new ApiFail(400, 'bad_request', '口座の型は、現金・預金・クレジットカード・電子マネーから選んでください。', 'wallet');
    const code = nextCode(store, 'w');
    const a = walletAccount(code, n, String(b.reading ?? ''), b.wallet.type, 1000 + Number(code.slice(1)), String(b.wallet.institution ?? ''));
    a.parent = b.parent || WALLET_PARENT[b.wallet.type];
    store.db.accounts.push(a);
    return a;
  }
  if (!KINDS.includes(b.kind)) throw new ApiFail(400, 'bad_request', '分類（資産・負債・資本・収益・費用）を選んでください。', 'kind');
  if (b.parent) {
    const p = store.db.accounts.find((a) => a.code === b.parent);
    if (!p || p.kind !== b.kind) throw new ApiFail(422, 'unknown_account', '親の科目は、同じ分類の科目から選んでください。', 'parent');
  }
  const code = nextCode(store, 'x');
  const a = {
    code,
    name: n,
    reading: String(b.reading ?? ''),
    kind: b.kind,
    normal_side: b.kind === 'asset' || b.kind === 'expense' ? 'debit' : 'credit',
    parent: b.parent || null,
    wallet: null,
    default_tax: tax(b.default_tax),
    default_pct: pct(b.default_pct),
    form_line: null,
    enabled: true,
    system: false,
    sort: 1000 + Number(code.slice(1)),
  };
  store.db.accounts.push(a);
  return a;
}

/** PATCH /accounts/{code}。名前を変えられないのは owner_draw owner_inv capital だけ。必須の科目は無効にできない。 */
export function patchAccount(store, code, body) {
  const a = store.db.accounts.find((x) => x.code === code);
  if (!a) throw new ApiFail(404, 'not_found', '科目が見つかりません。一覧を読み込み直してください。', null);
  const b = body || {};
  if ('kind' in b || 'wallet' in b) throw new ApiFail(409, 'bad_request', '分類と口座の型は、あとから変えられません。', 'kind');
  const next = { ...a };
  if ('name' in b) {
    const n = name(b.name);
    if (n !== a.name && PROTECTED_CODES.has(a.code)) throw new ApiFail(409, 'bad_request', `「${a.name}」は帳簿の決まりで使う科目なので、名前を変えられません。`, 'name');
    next.name = n;
  }
  if ('reading' in b) next.reading = String(b.reading ?? '');
  if ('enabled' in b) {
    if (b.enabled === false && REQUIRED_CODES.has(a.code)) throw new ApiFail(409, 'bad_request', `「${a.name}」は帳簿に必ず要る科目なので、無効にできません。`, 'enabled');
    next.enabled = Boolean(b.enabled);
  }
  if ('default_tax' in b) next.default_tax = tax(b.default_tax);
  if ('default_pct' in b) next.default_pct = pct(b.default_pct);
  Object.assign(a, next);
  return a;
}

function partnerFields(store, b, id) {
  const n = name(b.name);
  if (store.db.partners.some((p) => p.id !== id && p.name === n)) throw new ApiFail(409, 'duplicate_name', `「${n}」という取引先が、すでにあります。別の名前にするか、その取引先を直してください。`, 'name');
  const inv = b.invoice_number === null || b.invoice_number === undefined || b.invoice_number === '' ? null : String(b.invoice_number);
  if (inv && !INVOICE.test(inv)) throw new ApiFail(422, 'bad_invoice_number', '登録番号は、半角の T と13桁の数字で入れてください（例: T1234567890123）。', 'invoice_number');
  return { name: n, reading: String(b.reading ?? ''), invoice_number: inv, note: String(b.note ?? ''), enabled: b.enabled !== false };
}

/** POST /partners */
export function addPartner(store, body) {
  const p = { id: store.db.nextPartnerId++, ...partnerFields(store, body || {}, null) };
  store.db.partners.push(p);
  return p;
}

/** PATCH /partners/{id} */
export function patchPartner(store, id, body) {
  const p = store.db.partners.find((x) => x.id === Number(id));
  if (!p) throw new ApiFail(404, 'not_found', '取引先が見つかりません。一覧を読み込み直してください。', null);
  Object.assign(p, partnerFields(store, { ...p, ...(body || {}) }, p.id));
  return p;
}

/** GET /opening */
export function getOpening(store) {
  const s = store.openingState();
  if (!s) return { date: store.db.settings.opened_on || null, balances: [], capital: 0, entry_id: null };
  const kinds = new Map(store.db.accounts.map((a) => [a.code, a.kind]));
  const balances = s.entry.lines
    .filter((l) => kinds.get(l.account) === 'asset' || kinds.get(l.account) === 'liability')
    .map((l) => ({ account: l.account, amount: l.amount }));
  const cap = s.entry.lines.find((l) => l.account === 'capital');
  return { date: s.entry.date, balances, capital: cap ? (cap.side === 'credit' ? cap.amount : -cap.amount) : 0, entry_id: s.id };
}

/** PUT /opening。資産と負債の科目だけ。元入金は差額。0円の行は入れず、残らなければ仕訳を作らない（既存は取り消す）。 */
export function putOpening(store, body) {
  const b = body || {};
  const date = b.date || store.db.settings.opened_on;
  if (!isDate(date)) throw new ApiFail(400, 'bad_request', '期首残高の日付は YYYY-MM-DD の形にしてください。', 'date');
  const firstYear = (store.db.settings.opened_on || date).slice(0, 4);
  if (date.slice(0, 4) !== firstYear) throw new ApiFail(422, 'bad_request', `期首残高の日付は、最初の会計年度（${firstYear}年）の中にしてください。`, 'date');
  const seen = new Set();
  const lines = [];
  let assets = 0;
  let liabilities = 0;
  for (const row of Array.isArray(b.balances) ? b.balances : []) {
    const a = store.db.accounts.find((x) => x.code === row.account);
    if (!a || (a.kind !== 'asset' && a.kind !== 'liability') || a.code === 'owner_draw' || a.code === 'owner_inv') {
      throw new ApiFail(400, 'bad_request', '期首残高に入れられるのは、資産と負債の科目だけです（事業主貸と事業主借は除く）。', 'balances');
    }
    if (seen.has(a.code)) throw new ApiFail(400, 'bad_request', `「${a.name}」が2回入っています。1行にまとめてください。`, 'balances');
    seen.add(a.code);
    if (!Number.isInteger(row.amount) || row.amount < 0 || row.amount > 9999999999999) throw new ApiFail(422, 'amount_range', '残高は0円以上の整数にしてください。', 'balances');
    if (!row.amount) continue;
    const side = a.kind === 'asset' ? 'debit' : 'credit';
    if (side === 'debit') assets += row.amount;
    else liabilities += row.amount;
    lines.push({ account: a.code, account_name: a.name, side, amount: row.amount, tax_code: null, tax_amount: 0, memo: '' });
  }
  const cur = store.openingState();
  if (!lines.length) {
    if (cur) store.voidEntry(cur.id, '期首残高なし', { expectedRevision: cur.revision, allowOpening: true });
    return getOpening(store);
  }
  const capital = assets - liabilities;
  if (capital) lines.push({ account: 'capital', account_name: '元入金', side: capital > 0 ? 'credit' : 'debit', amount: Math.abs(capital), tax_code: null, tax_amount: 0, memo: '' });
  store.writeOpening(date, lines);
  return getOpening(store);
}
