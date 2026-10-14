// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引の登録の候補。科目は、種別に合う分類を上に並べる（支出は費用、収入は収益と事業主借）。
// 無効の科目は候補に出さない（過去の仕訳の表示には使う）。

const KIND_LABEL = { asset: '資産', liability: '負債', equity: '資本', revenue: '収益', expense: '費用' };
export const WALLET_TYPE_LABEL = { cash: '現金', bank: '預金', card: 'カード', ewallet: '電子マネー' };

function rank(a, kind) {
  if (kind === 'income') {
    if (a.kind === 'revenue') return 0;
    if (a.code === 'owner_inv') return 1;
    if (a.wallet) return 3;
    if (a.kind === 'expense') return 4;
    return 2;
  }
  if (a.kind === 'expense') return 0;
  if (a.code === 'owner_draw') return 1;
  if (a.wallet) return 3;
  if (a.kind === 'revenue') return 4;
  return 2;
}

/** 科目の候補（combobox の形）。 */
export function accountItems(accounts, kind) {
  return accounts
    .filter((a) => a.enabled)
    .map((a) => ({ a, r: rank(a, kind) }))
    .sort((x, y) => x.r - y.r || x.a.sort - y.a.sort)
    .map(({ a }) => ({
      value: a.code,
      label: a.name,
      reading: a.reading,
      hint: a.wallet ? `口座・${WALLET_TYPE_LABEL[a.wallet.type] || ''}` : KIND_LABEL[a.kind] || '',
    }));
}

/** 口座の候補。 */
export function walletItems(accounts) {
  return accounts
    .filter((a) => a.enabled && a.wallet)
    .sort((x, y) => x.sort - y.sort)
    .map((a) => ({ value: a.code, label: a.name, reading: a.reading, hint: WALLET_TYPE_LABEL[a.wallet.type] || '' }));
}

/** 取引先の候補（自由入力の欄なので、value は名前）。 */
export function partnerItems(partners) {
  return partners.filter((p) => p.enabled !== false).map((p) => ({ value: p.name, label: p.name, reading: p.reading }));
}

const LAST_WALLET_KEY = 'kamoku.entry.wallet';

/** 前に使った口座（無ければ最初の口座）。続けて入れるときの手間を減らすため。 */
export function defaultWallet(accounts) {
  const wallets = walletItems(accounts);
  let last = null;
  try {
    last = localStorage.getItem(LAST_WALLET_KEY);
  } catch {
    last = null;
  }
  return (wallets.find((w) => w.value === last) || wallets[0] || { value: null }).value;
}

/** 使った口座を覚える。 */
export function rememberWallet(code) {
  try {
    if (code) localStorage.setItem(LAST_WALLET_KEY, code);
  } catch {
    // 覚えられなくても、次は最初の口座になるだけ。
  }
}
