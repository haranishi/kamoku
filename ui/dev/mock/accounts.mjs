// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 模擬サーバーの勘定科目。コードと分類は、R1の依頼書の「標準の科目」と同じにしてある。
// 本物のサーバーができたら、このファイルごと捨てる（一時の道具）。

// [コード, 名前, 読み, 分類, 既定の税区分, 有効]
const STANDARD = [
  ['cash', '現金', 'げんきん', 'asset', null, true],
  ['dep_current', '当座預金', 'とうざよきん', 'asset', null, false],
  ['dep_fixed', '定期預金', 'ていきよきん', 'asset', null, false],
  ['dep_other', '普通預金・その他の預金', 'ふつうよきんそのたのよきん', 'asset', null, true],
  ['notes_recv', '受取手形', 'うけとりてがた', 'asset', null, false],
  ['ar', '売掛金', 'うりかけきん', 'asset', null, true],
  ['securities', '有価証券', 'ゆうかしょうけん', 'asset', null, false],
  ['inventory', '棚卸資産', 'たなおろししさん', 'asset', null, true],
  ['prepaid', '前払金', 'まえばらいきん', 'asset', null, true],
  ['loan_recv', '貸付金', 'かしつけきん', 'asset', null, false],
  ['bldg', '建物', 'たてもの', 'asset', null, false],
  ['bldg_fix', '建物附属設備', 'たてものふぞくせつび', 'asset', null, false],
  ['mach', '機械装置', 'きかいそうち', 'asset', null, false],
  ['vehicle', '車両運搬具', 'しゃりょううんぱんぐ', 'asset', null, false],
  ['tools', '工具器具備品', 'こうぐきぐびひん', 'asset', null, true],
  ['land', '土地', 'とち', 'asset', null, false],
  ['owner_draw', '事業主貸', 'じぎょうぬしかし', 'asset', null, true],
  ['notes_pay', '支払手形', 'しはらいてがた', 'liability', null, false],
  ['ap', '買掛金', 'かいかけきん', 'liability', null, true],
  ['loan_pay', '借入金', 'かりいれきん', 'liability', null, true],
  ['accrued', '未払金', 'みばらいきん', 'liability', null, true],
  ['adv_recv', '前受金', 'まえうけきん', 'liability', null, true],
  ['deposit_recv', '預り金', 'あずかりきん', 'liability', null, true],
  ['bad_debt_res', '貸倒引当金', 'かしだおれひきあてきん', 'liability', null, false],
  ['owner_inv', '事業主借', 'じぎょうぬしかり', 'liability', null, true],
  ['capital', '元入金', 'もといれきん', 'equity', null, true],
  ['sales', '売上高', 'うりあげだか', 'revenue', 'S10', true],
  ['misc_income', '雑収入', 'ざっしゅうにゅう', 'revenue', null, true],
  ['home_use', '家事消費', 'かじしょうひ', 'revenue', null, false],
  ['exp_purchase', '仕入高', 'しいれだか', 'expense', 'P10', true],
  ['exp_tax', '租税公課', 'そぜいこうか', 'expense', null, true],
  ['exp_freight', '荷造運賃', 'にづくりうんちん', 'expense', 'P10', true],
  ['exp_utility', '水道光熱費', 'すいどうこうねつひ', 'expense', 'P10', true],
  ['exp_travel', '旅費交通費', 'りょひこうつうひ', 'expense', null, true],
  ['exp_comm', '通信費', 'つうしんひ', 'expense', 'P10', true],
  ['exp_ads', '広告宣伝費', 'こうこくせんでんひ', 'expense', 'P10', true],
  ['exp_entertain', '接待交際費', 'せったいこうさいひ', 'expense', 'P10', true],
  ['exp_insurance', '損害保険料', 'そんがいほけんりょう', 'expense', 'NT', true],
  ['exp_repair', '修繕費', 'しゅうぜんひ', 'expense', 'P10', true],
  ['exp_supplies', '消耗品費', 'しょうもうひんひ', 'expense', 'P10', true],
  ['exp_depr', '減価償却費', 'げんかしょうきゃくひ', 'expense', null, true],
  ['exp_welfare', '福利厚生費', 'ふくりこうせいひ', 'expense', null, true],
  ['exp_salary', '給料賃金', 'きゅうりょうちんぎん', 'expense', null, true],
  ['exp_outsource', '外注費', 'がいちゅうひ', 'expense', 'P10', true],
  ['exp_interest', '利子割引料', 'りしわりびきりょう', 'expense', 'NT', true],
  ['exp_rent', '地代家賃', 'ちだいやちん', 'expense', null, true],
  ['exp_baddebt', '貸倒金', 'かしだおれきん', 'expense', null, true],
  ['exp_misc', '雑費', 'ざっぴ', 'expense', null, true],
  ['exp_meeting', '会議費', 'かいぎひ', 'expense', 'P10', true],
  ['exp_books', '新聞図書費', 'しんぶんとしょひ', 'expense', null, true],
  ['exp_dues', '諸会費', 'しょかいひ', 'expense', null, true],
  ['exp_vehicle', '車両費', 'しゃりょうひ', 'expense', 'P10', true],
  ['exp_fee', '支払手数料', 'しはらいてすうりょう', 'expense', 'P10', true],
];

// 標準の科目は、すべて system: true（spec-issues の R1-11 と、2026-10-14 20:10 の決定）。
// 名前を変えられないのは、この3つだけ（PATCH の名前の変更を409で断る）。
export const PROTECTED_CODES = new Set(['owner_draw', 'owner_inv', 'capital']);
// 帳簿に必ず要る科目。無効にできない（kamoku-books の REQUIRED_ACCOUNTS と同じ）。
export const REQUIRED_CODES = new Set(['cash', 'dep_other', 'accrued', 'owner_draw', 'owner_inv', 'capital', 'sales']);
const BASE_SORT = { asset: 100, liability: 200, equity: 300, revenue: 400, expense: 500 };
/** 口座の型ごとの親の科目（docs/accounts.md）。 */
export const WALLET_PARENT = { cash: 'cash', ewallet: 'cash', bank: 'dep_other', card: 'accrued' };
const NORMAL_SIDE = { asset: 'debit', expense: 'debit', liability: 'credit', equity: 'credit', revenue: 'credit' };

// 架空の利用者が決めた既定の事業割合（AC-03.3 の例: 車両費は30%）。標準はすべて100。
const DEMO_DEFAULT_PCT = { exp_comm: 70, exp_vehicle: 30 };

/** 口座の科目を1つ作る（合成データと、POST /accounts の両方で使う）。 */
export function walletAccount(code, name, reading, type, sort, institution = '') {
  return {
    code,
    name,
    reading,
    kind: type === 'card' ? 'liability' : 'asset',
    normal_side: type === 'card' ? 'credit' : 'debit',
    parent: WALLET_PARENT[type],
    wallet: { type, institution },
    default_tax: null,
    default_pct: 100,
    form_line: null,
    enabled: true,
    system: false,
    sort,
  };
}

/** 標準の科目と、合成データの口座3つ（withWallets が偽なら、口座なし。初期設定の前の帳簿）。 */
export function makeAccounts({ withWallets = true } = {}) {
  const counters = {};
  const accounts = STANDARD.map(([code, name, reading, kind, tax, enabled]) => {
    counters[kind] = (counters[kind] || 0) + 1;
    return {
      code,
      name,
      reading,
      kind,
      normal_side: NORMAL_SIDE[kind],
      parent: null,
      wallet: null,
      default_tax: tax,
      default_pct: DEMO_DEFAULT_PCT[code] || 100,
      form_line: kind === 'revenue' || kind === 'expense' ? name : null,
      enabled,
      system: true,
      sort: BASE_SORT[kind] + (counters[kind] - 1) * 10,
    };
  });
  if (!withWallets) return accounts;
  accounts.push(
    walletAccount('w01', '現金', 'げんきん', 'cash', 1001),
    walletAccount('w02', 'デモ銀行 普通', 'でもぎんこう', 'bank', 1002),
    walletAccount('w03', 'サンプルカード', 'さんぷるかーど', 'card', 1003),
  );
  return accounts;
}

const KIND_ORDER = { asset: 0, liability: 1, equity: 2, revenue: 3, expense: 4 };

/** 科目の並び（分類、sort、コードの順。spec-issues の R1-12）。 */
export function chartOrder(a, b) {
  return KIND_ORDER[a.kind] - KIND_ORDER[b.kind] || a.sort - b.sort || (a.code < b.code ? -1 : a.code > b.code ? 1 : 0);
}
