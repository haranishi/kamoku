// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// かんたん入力の、保存の前の検査。誤りは欄のすぐ下に出す（entry-simple.js が、最初の誤りの欄へフォーカスを移す）。

import { WALLET_LABEL } from './entry-simple-fields.js';

/**
 * 誤りのあった欄の名前（上から順、同じ欄は1つ）。保存の帯の一言「2か所を直してください：金額、科目」に使う。
 * @param {{ u: object, s: object }} p
 * @param {HTMLElement[]} problems checkSimple の結果
 */
export function fieldNames({ u, s }, problems) {
  const names = new Map([
    [u.dateIn.input, '日付'],
    [u.amountIn.input, '金額'],
    [u.accountCb.input, '科目'],
    [u.toWalletCb.input, '移動先の口座'],
    [u.walletCb.input, WALLET_LABEL[s.kind]],
    [u.partnerCb.input, '取引先'],
    [u.pctIn, '事業の割合'],
  ]);
  return [...new Set(problems)].map((el) => names.get(el)).filter(Boolean);
}

/**
 * @param {{ u: object, s: object, pctProblem: () => string }} p u は entry-simple-fields.js の欄、s は入力の状態
 * @returns {HTMLElement[]} 誤りのある欄（並びの上から順）。空なら保存してよい
 */
export function checkSimple({ u, s, pctProblem }) {
  const problems = [];
  if (!u.dateIn.commit()) problems.push(u.dateIn.input);
  if (u.amountIn.input.value.trim() === '') {
    u.amountF.setError('金額を入れてください。');
    problems.push(u.amountIn.input);
  } else if (!u.amountIn.commit()) problems.push(u.amountIn.input);
  const pick = (cb, f, what) => {
    f.setError('');
    if (cb.value) return;
    f.setError(cb.input.value.trim() ? `打った文字に合う${what}がありません。候補から選んでください。` : `${what}を選んでください。`);
    problems.push(cb.input);
  };
  if (s.kind === 'transfer') pick(u.toWalletCb, u.toWalletF, '移動先の口座');
  else pick(u.accountCb, u.accountF, '科目');
  pick(u.walletCb, u.walletF, '口座');
  if (s.kind === 'transfer' && s.wallet && s.wallet === s.to_wallet) {
    u.toWalletF.setError('移動元と移動先が同じ口座です。別の口座を選んでください。');
    problems.push(u.toWalletCb.input);
  }
  // 取引先: 候補に無い打ちかけの名前を、黙って新しい取引先にしない（候補の一覧の最後の行で足したときだけ使う）。
  const name = u.partnerCb.input.value.trim();
  const unsure = Boolean(name && !u.partnerCb.item);
  u.partnerF.setError(unsure ? `「${name}」は候補にありません。候補から選ぶか、一覧の最後の行で足してください。` : '');
  if (unsure) problems.push(u.partnerCb.input);
  const pct = pctProblem();
  u.pctF.setError(pct);
  if (pct) problems.push(u.pctIn);
  return problems;
}
