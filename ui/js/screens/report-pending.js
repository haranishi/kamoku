// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票とホームに出す「未確認は集計に入っていない」の注記（SC-06、SC-01）。
// 取り込んだ明細は、確定するまで仕訳ではないので、どの帳票にも残高にも入らない。それを知らずに数字を読むと、
// 未確認の分だけ少なく見えたまま気づけないので、件数（と出金・入金の合計）を数字の近くに書く。0件なら何も出さない。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { formatYen } from '../format.js';
import { phrase } from '../components/phrase.js';

const PAGE = 500;
// これより多い（1万件を超える）ときは、合計を数えずに件数だけを出す（読み込みを長引かせないため）。
const MAX_PAGES = 20;

/**
 * 期間（と口座）の中の未確認を数える。
 * @param {{ get: (path: string, params?: object) => Promise<any> }} api
 * @param {{ from?: string, to?: string, wallet?: string }} range
 * @returns {Promise<{ count: number, out: number|null, in: number|null }>} 合計を数えきれなかったら out と in は null
 */
export async function countPending(api, range) {
  let out = 0;
  let inn = 0;
  let offset = 0;
  let total = 0;
  for (let page = 0; page < MAX_PAGES; page += 1) {
    const res = await api.get('/drafts', { status: 'draft', ...range, limit: PAGE, offset });
    total = res.total;
    for (const d of res.items) {
      if (d.direction === 'out') out += d.amount;
      else inn += d.amount;
    }
    offset += res.items.length;
    if (!res.items.length || offset >= total) return { count: total, out, in: inn };
  }
  return { count: total, out: null, in: null };
}

/**
 * 数と単位の塊（「未確認 3件」「（出金 1,650円・」）。途中で折らず、空白は詰めずに見せる（phrase は空白で塊を切り、
 * 塊の末尾の空白は見えなくなる）。後ろに折ってよい切れ目（<wbr>）を置く。
 */
const keep = (text) => [h('span', { class: 'nw' }, text.replace(/ /g, '\u00a0')), h('wbr')];

/**
 * 帳票の期間の下に出す1行。「未確認 3件（出金 1,650円・入金 0円）は、この集計に入っていません。未確認を開く」。
 * @param {{ count: number, out: number|null, in: number|null }} info
 * @param {string} [href] 未確認の画面（口座の元帳なら、その口座で絞った未確認）
 * @returns {HTMLElement|null} 0件なら null
 */
export function pendingNote(info, href = '#/inbox') {
  if (!info || !info.count) return null;
  // 「は、」は前の塊に付ける（行の頭に助詞が来ないように）。
  const head = info.out === null
    ? [keep(`未確認 ${formatYen(info.count)}件は、`)]
    : [keep(`未確認 ${formatYen(info.count)}件`), keep(`（出金 ${formatYen(info.out)}円・`), keep(`入金 ${formatYen(info.in)}円）は、`)];
  return h(
    'p',
    { class: 'pending-note' },
    icon('info', { size: 16 }),
    h('span', null, head, phrase('この集計に入っていません。'), ' ', h('a', { href, class: 'tap' }, '未確認を開く')),
  );
}

/** ホームの「口座の残高」の見出しの近くに出す、短い注記。0件なら null。 */
export function pendingShort(count) {
  if (!count) return null;
  return h('p', { class: 'pending-note pending-note--short' }, icon('info', { size: 16 }), h('span', null, keep(`未確認 ${formatYen(count)}件は、`), phrase('この残高に入っていません。')));
}
