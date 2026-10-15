// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 未確認の行の決まり（SC-03）。DOMを使わないので Node で試せる（ui/tests/inbox-helpers.test.mjs）。
// 税区分の決まりは、利用者が手で選んだかどうかで分ける。前の決まり（いまの値が前の科目の既定と同じときだけ入れ直す）では、
// 入金の行で通信費（課税仕入10%は入金の行で選べないので空になる）から売上高へ選び直すと、空が利用者の値とみなされて残り、
// 売上が「対象外」で記帳されかけた（体験の評価 E2）。

import { taxOptions, TAX_LABELS } from '../account-defaults.js';
import { formatYen } from '../format.js';

/** 行で使う、科目の既定の税区分。行の向きで選べない値（入金の行の課税仕入など）、口座、免税なら null。 */
export function rowDefaultTax(account, kind, exempt) {
  if (exempt || !account || account.wallet) return null;
  const code = account.default_tax ?? null;
  return code && taxOptions(kind).some((x) => x.value === code) ? code : null;
}

/**
 * 科目を変えた後の税区分。手で選んでいなければ、新しい科目の既定に入れ直す（既定が無ければ null）。手で選んだ値は残す。
 * 相手が口座（振替）か免税なら、いつも null（税区分を使わない）。
 * @param {{ next: object|null, kind: 'income'|'expense', exempt: boolean, tax: string|null, touched: boolean }} p
 */
export function taxAfterAccount({ next, kind, exempt, tax, touched }) {
  if (exempt || (next && next.wallet)) return null;
  return touched ? tax ?? null : rowDefaultTax(next, kind, exempt);
}

/** 科目の既定と違う税区分か（科目があり、口座でも免税でもないときだけ比べる）。 */
export function taxDiffers({ account, kind, exempt, tax }) {
  if (exempt || !account || account.wallet) return false;
  return (tax ?? null) !== rowDefaultTax(account, kind, exempt);
}

/**
 * 読み込んだ行の税区分を、手で選んだ値とみなすか。読み込んだ時点では、選んだ経緯が分からないので、
 * 科目の既定と違う値（科目が無ければ、空でない値）を、手で選んだ値とする。
 */
export function touchedOnLoad({ account, kind, exempt, tax }) {
  if (exempt) return false;
  if (!account) return (tax ?? null) !== null;
  return taxDiffers({ account, kind, exempt, tax });
}

/** 税区分の表示名（空は「対象外」）。 */
export function taxLabel(code) {
  return code ? TAX_LABELS[code] || code : '対象外';
}

/**
 * 税区分の選択欄の中の名前。取引の登録・訂正の画面と同じ名前（TAX_LABELS）にする（前は「課税仕入8%軽減」「経過措置10%」と
 * 画面で違った。E2 第2回）。表の狭い列に入らない経過措置の2つだけ、後ろの「（免税事業者等から）」を省く（欄の title に全体を出す）。
 */
export function taxChoiceLabel(code) {
  return taxLabel(code).replace('（免税事業者等から）', '');
}

/**
 * 手で選んでいない空の税区分の、仮の値の名前と説明。空のまま確定すると「対象外」で記帳される（確定のとき、サーバーは科目の
 * 既定で埋めない）ので、科目があれば実際の値「対象外」を出す（「科目で決まる」だけでは、何になるかが title でしか分からなかった。
 * E2 第2回）。科目の既定が対象外（勘定科目の画面でも「対象外」と出る科目）なら「対象外（既定）」。「対象外（科目で決まる）」は
 * 幅1440の表の税区分の欄（文字の入る幅134px）に入らない（154px）ので、短い「（既定）」にした。科目の既定が行の向きで使えない
 * （入金の行の通信費など）か、科目の一覧に無いときは、既定とは言えないので「対象外」だけにする。
 * @param {{ account: object|null, hasAccount: boolean, kind: 'income'|'expense' }} p
 *   account は選んである科目（科目の一覧に無いときは null）、hasAccount は科目が選んであるか。
 */
export function unsetTaxText({ account, hasAccount, kind }) {
  if (!hasAccount) return { label: '科目で決まる', title: '科目を選ぶと、科目の既定の税区分が入ります' };
  const code = account && !account.wallet ? account.default_tax ?? null : null;
  if (account && !code) return { label: '対象外（既定）', title: 'この科目の既定の税区分は「対象外」です。このまま確定すると「対象外」になります。' };
  const title = code
    ? `この科目の既定の税区分（${taxLabel(code)}）は、${kind === 'income' ? '入金' : '出金'}の行では使えません。このまま確定すると「対象外」になります。`
    : 'このまま確定すると「対象外」になります。';
  return { label: '対象外', title };
}

/** 「税区分が、科目の既定（課税売上10%）と違います。」 */
export function taxDiffText(account, kind, exempt) {
  return `税区分が、科目の既定（${taxLabel(rowDefaultTax(account, kind, exempt))}）と違います。`;
}

/**
 * 向きと逆の科目の注意（出金の行に収益、入金の行に費用）。返金などでは正しいので、確定は止めない。それ以外は null。
 * @param {{ kind?: string }|null} account
 * @param {'in'|'out'} direction
 */
export function directionWarning(account, direction) {
  if (!account) return null;
  if (direction === 'out' && account.kind === 'revenue') return '出金の行に、収益の科目が選ばれています。返金などでなければ、科目を見直してください。';
  if (direction === 'in' && account.kind === 'expense') return '入金の行に、費用の科目が選ばれています。返金などでなければ、科目を見直してください。';
  return null;
}

/**
 * 選択欄で上下の矢印を押したときの行き先。選べない（disabled・hidden）選択肢は飛ばし、端では止まる。動かないなら -1。
 * @param {Array<{ disabled?: boolean, hidden?: boolean }>} options
 * @param {number} index いまの位置
 * @param {1|-1} dir
 */
export function stepOption(options, index, dir) {
  for (let i = index + dir; i >= 0 && i < options.length; i += dir) {
    if (!options[i].disabled && !options[i].hidden) return i;
  }
  return -1;
}

/**
 * 行を見分ける名前「09/18 デモ電器 7,470円」。欄の読み上げの名前に足す（同じ摘要の行が並ぶと、「日付 デモ電器」が重なった）。
 * 日付は今年なら年を省く（表の日付の欄と同じ）。
 */
export function rowLabel(draft, thisYear) {
  const iso = String(draft.date || '');
  const day = iso.slice(0, 4) === thisYear ? `${iso.slice(5, 7)}/${iso.slice(8, 10)}` : iso.replace(/-/g, '/');
  return `${day} ${draft.description || '摘要なし'} ${formatYen(draft.amount)}円`;
}
