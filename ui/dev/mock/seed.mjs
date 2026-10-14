// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 合成データ。名前はすべて架空（「サンプル」「デモ」を含む）。乱数の種を固定して、毎回同じ帳簿にする。
// 今日を含む12か月分、約60仕訳、口座3つ、取引先5つ。

import { makeAccounts } from './accounts.mjs';
import { createStore } from './store.mjs';
import { fingerprint } from './imports.mjs';

/** 種を固定した疑似乱数（mulberry32）。 */
export function prng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pad2 = (n) => String(n).padStart(2, '0');

function shiftMonth(ym, n) {
  const [y, m] = ym.split('-').map(Number);
  const t = new Date(Date.UTC(y, m - 1 + n, 1));
  return `${t.getUTCFullYear()}-${pad2(t.getUTCMonth() + 1)}`;
}

const PARTNERS = [
  ['サンプル商事', 'さんぷるしょうじ'],
  ['デモ企画', 'でもきかく'],
  ['サンプル通信', 'さんぷるつうしん'],
  ['デモ不動産', 'でもふどうさん'],
  ['サンプル文具', 'さんぷるぶんぐ'],
];

/** 初期設定の前の帳簿。設定の既定値は本物の保存の層（kamoku-store の Settings::default）に合わせる。 */
function freshStore() {
  let n = 0;
  return createStore({
    settings: { business_name: '', owner_name: '', tax_mode: 'exempt', invoice_number: null, rounding: 'floor', opened_on: '', locked_until: null, setup_done: false },
    accounts: makeAccounts({ withWallets: false }),
    partners: [],
    templates: [],
    drafts: [],
    idSource: () => {
      n += 1;
      return `e_${String(n).padStart(20, '0')}`;
    },
    clock: () => Math.floor(Date.now() / 1000),
  });
}

/**
 * 模擬の帳簿を作る。
 * @param {{ today: string, taxMode?: 'exempt'|'taxable', empty?: boolean, fresh?: boolean, seed?: number }} opts
 */
export function seed({ today, taxMode = 'taxable', empty = false, fresh = false, seed: seedValue = 20261014 }) {
  if (fresh) return freshStore();
  const rand = prng(seedValue);
  const pick = (min, max, step = 10) => min + Math.floor(rand() * ((max - min) / step + 1)) * step;
  const thisMonth = today.slice(0, 7);
  const firstMonth = shiftMonth(thisMonth, -11);
  let clockSec = Math.floor(Date.UTC(Number(firstMonth.slice(0, 4)), Number(firstMonth.slice(5, 7)) - 1, 1) / 1000);
  const idSource = () => {
    let hex = '';
    for (let i = 0; i < 20; i += 1) hex += Math.floor(rand() * 16).toString(16);
    return `e_${hex}`;
  };

  const settings = {
    business_name: 'サンプル事務所',
    owner_name: '',
    tax_mode: taxMode,
    invoice_number: null,
    rounding: 'floor',
    opened_on: `${firstMonth}-01`,
    locked_until: null,
    setup_done: true,
  };
  const partners = PARTNERS.map(([name, reading], i) => ({ id: i + 1, name, reading, invoice_number: null, note: '', enabled: true }));
  const templates = [
    { id: 1, name: '携帯電話の利用料', body: { kind: 'expense', amount: 11000, account: 'exp_comm', wallet: 'w03', tax_code: 'P10', partner: 'サンプル通信', description: '携帯電話の利用料', business_pct: 70, memo: '' } },
    { id: 2, name: '事務所兼自宅の家賃', body: { kind: 'expense', amount: 66000, account: 'exp_rent', wallet: 'w02', tax_code: 'NT', partner: 'デモ不動産', description: '家賃', business_pct: 50, memo: '' } },
  ];
  const drafts = makeDrafts(today, thisMonth);
  const store = createStore({
    settings,
    accounts: makeAccounts(),
    partners,
    templates,
    drafts,
    idSource,
    clock: () => Math.floor(Date.now() / 1000),
  });
  if (empty) return store;

  // 記録の時刻は、取引の日付の夕方にそろえる（時刻は信用しない前提の、見た目のためだけの値）。
  const at = (date) => {
    const [y, m, d] = date.split('-').map(Number);
    clockSec = Math.max(clockSec + 60, Math.floor(Date.UTC(y, m - 1, d, 9, 30) / 1000));
    return clockSec;
  };
  const post = (input) => store.post(input, { source: 'demo', recordedAt: at(input.date) });

  store.post(
    {
      date: `${firstMonth}-01`,
      description: '期首残高',
      partner: '',
      lines: [
        { account: 'w01', side: 'debit', amount: 30000, tax_code: null, tax_amount: 0, memo: '' },
        { account: 'w02', side: 'debit', amount: 800000, tax_code: null, tax_amount: 0, memo: '' },
        { account: 'capital', side: 'credit', amount: 830000, tax_code: null, tax_amount: 0, memo: '' },
      ],
      meta: {},
    },
    { source: 'opening', recordedAt: at(`${firstMonth}-01`), opening: true },
  );

  const exp = (date, account, wallet, amount, extra = {}) =>
    post({ kind: 'expense', date, amount, account, wallet, tax_code: 'P10', partner: '', description: '', business_pct: 100, memo: '', ...extra });
  let supplyEntry = null;
  let cardSpent = 0;
  // 赤字の月（売上の入金が無く、大きな買い物がある）。グラフで負の利益の描き方を確かめるため、今年の中に置く。
  const LOSS = 7;

  for (let i = 0; i < 12; i += 1) {
    const ym = shiftMonth(firstMonth, i);
    const m = Number(ym.slice(5, 7));
    const has = (day) => `${ym}-${pad2(day)}` <= today;
    const d = (day) => `${ym}-${pad2(day)}`;
    // 今月は必ず数件あるようにする（ホームの「経費の上位」が空にならないように）。
    const maybe = (p) => i === 11 || rand() < p;
    let cardThisMonth = 0;
    const card = (amount) => {
      cardThisMonth += amount;
      return amount;
    };

    if (has(3) && maybe(0.45)) {
      const e = exp(d(3), 'exp_supplies', 'w03', card(pick(1100, 6600, 110)), { partner: 'サンプル文具', description: '事務用品' });
      if (!supplyEntry) supplyEntry = e;
    }
    if (has(5)) exp(d(5), 'exp_comm', 'w03', card(11000), { partner: 'サンプル通信', description: `${m}月分 携帯電話`, business_pct: 70 });
    if (has(8) && maybe(0.2)) exp(d(8), 'exp_books', 'w03', card(pick(1650, 3300, 110)), { description: '技術書' });
    if (i > 0 && has(10) && cardSpent > 0) {
      post({ kind: 'transfer', date: d(10), amount: cardSpent, wallet: 'w02', to_wallet: 'w03', partner: '', description: 'カードの引き落とし', memo: '' });
    }
    if (i !== LOSS && has(10)) {
      const prevM = m === 1 ? 12 : m - 1;
      post({ kind: 'income', date: d(10), amount: pick(198000, 275000, 1100), account: 'sales', wallet: 'w02', tax_code: 'S10', partner: 'サンプル商事', description: `${prevM}月分 業務委託料`, business_pct: 100, memo: '' });
    }
    if (has(12) && maybe(0.3)) exp(d(12), 'exp_vehicle', 'w03', card(pick(4000, 7500, 100)), { partner: '', description: 'ガソリン', business_pct: 30 });
    if (has(13) && i % 2 === 1) exp(d(13), 'exp_utility', 'w02', pick(14000, 22000, 10), { description: '電気代（2か月分）', business_pct: 30 });
    if (has(16) && maybe(0.25)) exp(d(16), 'exp_travel', 'w01', pick(420, 1980, 10), { description: '電車代（打ち合わせ）' });
    if (i % 4 === 0 && has(20)) {
      post({ kind: 'income', date: d(20), amount: pick(33000, 88000, 1100), account: 'sales', wallet: 'w02', tax_code: 'S10', partner: 'デモ企画', description: '資料の作成', business_pct: 100, memo: '' });
    }
    if (has(21) && rand() < 0.2) exp(d(21), 'exp_meeting', 'w01', pick(880, 2640, 10), { description: '打ち合わせの飲み物' });
    if (i % 4 === 1 && has(22)) exp(d(22), 'exp_outsource', 'w02', pick(33000, 77000, 1100), { description: 'イラストの外注' });
    if (i % 6 === 2 && has(24)) post({ kind: 'transfer', date: d(24), amount: 30000, wallet: 'w02', to_wallet: 'w01', partner: '', description: '現金の引き出し', memo: '' });
    if (has(27)) exp(d(27), 'exp_rent', 'w02', 66000, { tax_code: 'NT', partner: 'デモ不動産', description: '家賃', business_pct: 50 });
    if (i === LOSS && has(15)) exp(d(15), 'exp_supplies', 'w03', card(46200), { partner: 'サンプル文具', description: 'モニターとキーボード' });
    if (i === 9 && has(6)) {
      const dup = exp(d(6), 'exp_comm', 'w03', 11000, { partner: 'サンプル通信', description: `${m}月分 携帯電話`, business_pct: 70 });
      store.voidEntry(dup.id, '二重に登録したため', { recordedAt: at(d(6)), expectedRevision: 1 });
    }
    cardSpent = cardThisMonth;
  }

  if (supplyEntry) {
    const fixed = { ...supplyEntry.input, amount: supplyEntry.input.amount + 1100 };
    store.revise(supplyEntry.id, fixed, '金額の入力ミス（レシートを見直した）', { recordedAt: at(supplyEntry.date), expectedRevision: 1 });
  }
  return store;
}

const DRAFT_ROWS = [
  ['w03', 'サンプルストア', 'out', 1650],
  ['w03', 'デモ鉄道', 'out', 420],
  ['w03', 'サンプルカフェ', 'out', 880],
  ['w03', 'デモ書店', 'out', 2970],
  ['w03', 'サンプル電気', 'out', 12800],
  ['w03', 'デモ石油', 'out', 5600],
  ['w03', 'サンプルストア', 'out', 2310],
  ['w03', 'デモ配送', 'out', 1200],
  ['w03', 'サンプルクラウド', 'out', 1980],
  ['w02', 'デモ銀行 手数料', 'out', 330],
  ['w02', 'サンプル保険', 'out', 15460],
  ['w02', 'デモ企画 振込', 'in', 55000],
];

// 取込で入った「未確認」。台帳の外にある。指紋と出現番号は、取込の重複の判定と同じ作り方（data-model.md の決まり4）。
function makeDrafts(today, thisMonth) {
  const base = (id, wallet, date, description, direction, amount, status) => ({ id, batch_id: 1, wallet, date, description, direction, amount, account: null, tax_code: null, partner: '', memo: '', business_pct: 100, status, entry_id: null });
  const rows = DRAFT_ROWS.map(([wallet, description, direction, amount], i) => {
    const day = Math.min(Number(today.slice(8, 10)), 1 + i);
    return base(i + 1, wallet, `${thisMonth}-${pad2(day)}`, description, direction, amount, 'draft');
  }).concat([
    base(13, 'w03', `${thisMonth}-01`, 'サンプルストア（返品）', 'in', 1650, 'ignored'),
    base(14, 'w02', `${thisMonth}-01`, 'デモ銀行 利息', 'in', 3, 'ignored'),
  ]);
  const seen = new Map();
  for (const d of rows) {
    d.fingerprint = fingerprint(d.wallet, d);
    d.occurrence = (seen.get(d.fingerprint) || 0) + 1;
    seen.set(d.fingerprint, d.occurrence);
  }
  return rows;
}
