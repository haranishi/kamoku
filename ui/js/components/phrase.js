// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 日本語の文を、語の途中で折らない「塊」に分けて並べる。行末に「反映していま／せん」の「せん」や、
// 「保存、一／の仕入先」の「一」（長音の「ー」に見える）だけが残らないようにするため。
// 塊は幅に収まる間はひと続きの span（.nw）にし、切れ目に <wbr> を置く。枠より長い塊は中でも折る。
// ブラウザの word-break: auto-phrase は使わない
// （語の途中で割ることがあり、iPhone の Safari が対応していない）。text-wrap: pretty は日本語の2文字の取り残しを直さない。

import { h } from '../dom.js';

const HIRA = /[ぁ-ゟ]/;
const ENDING = /[をにがはのでとへやもからてたなるばだ]/;
const SPACE = /[ 　]/;
const PUNCT = /[、。，．！？!?：:]/;
const CLOSE = /[）」』】］〉》)\]]/;
const OPEN = /[（「『【［〈《(\[]/;
/** 塊の長さの上限（幅390pxの帯の1行に、16pxの文字で収まる長さより短く）。 */
const MAX = 14;
/** 英数字の語（Shift_JIS、UTF-8、2026-10-14）をつくる文字と、塊の終わりにある語。 */
const WORD = /[A-Za-z0-9_.-]/;
const WORD_TAIL = /[A-Za-z0-9_.-]+$/;
/** 1つだけで行末に残ると読みにくい語は、次の塊と結ぶ（「その｜年の」→「その年の」）。 */
const STICKY = new Set(['この', 'その', 'あの', 'どの']);
const KANJI = /[一-鿿々]/;
/** 1字で漢字のあいだに挟まると、助詞として切ってよいひらがな。これ以外の1字（送り仮名）は、前後の漢字と結ぶ。 */
const PARTICLES = new Set(['の', 'を', 'に', 'は', 'が', 'と', 'で', 'も', 'へ', 'や', 'か']);

/** 「差し引く」「取り消す」の送り仮名（漢字＋ひらがな1字＋漢字）なら真。そこでは切らない。 */
function okurigana(cur, next) {
  const run = cur.slice(-1);
  return cur.length >= 2 && KANJI.test(cur.slice(-2, -1)) && KANJI.test(next) && !PARTICLES.has(run);
}

/**
 * 文を塊に分ける。切れ目は、句読点のあと、閉じ括弧のあと（助詞が続くときは結ぶ）、開き括弧の前、
 * ひらがなから漢字・カタカナ・英数字に変わる所（「簡易課税と｜特例、」）。送り仮名（「差し引く」）では切らない。
 * @param {string} text
 * @returns {string[]}
 */
export function chunks(text) {
  const out = [];
  let cur = '';
  const chars = [...String(text)];
  for (const [i, c] of chars.entries()) {
    const prev = cur.slice(-1);
    let cut = false;
    let carry = '';
    if (cur) {
      const joins = PUNCT.test(c) || CLOSE.test(c);
      if (SPACE.test(c)) cut = false;
      else if (SPACE.test(prev)) cut = true;
      // 「開業日（または、」はひと続きにし、短い補足の前で名前を切らない。
      else if (OPEN.test(c)) cut = !HIRA.test(chars[i + 1] || '') || cur.length > 4;
      else if (PUNCT.test(prev)) cut = !joins;
      else if (CLOSE.test(prev)) cut = !joins && !HIRA.test(c);
      else if (HIRA.test(prev)) {
        cut = ENDING.test(prev) && !joins && !HIRA.test(c) && c !== 'ー' && !STICKY.has(cur) && !okurigana(cur, c);
        // 「分にはこの｜特例を」は「分には｜この特例を」にする（「この」を行末に残さない）。
        if (cut) carry = [...STICKY].find((w) => cur.endsWith(w) && cur.length > w.length) || '';
      }
    }
    // 長さの上限で切るとき、英数字の語（Shift_JIS・UTF-8 など）を真ん中で割らず、語ごと次の塊へ送る。
    // 塊の全体がその語なら（長すぎる識別子）、送っても同じなので、その場で切る。
    if (!cut && cur.length >= MAX && WORD.test(prev) && WORD.test(c)) {
      const word = (cur.match(WORD_TAIL) || [''])[0];
      if (word.length < cur.length) carry = word;
    }
    if (cut || cur.length >= MAX) {
      out.push(carry ? cur.slice(0, -carry.length) : cur);
      cur = carry;
    }
    cur += c;
  }
  if (cur) out.push(cur);
  return out;
}

/**
 * 文を、塊ごとの span と <wbr> を持つ1つの span にして返す。
 * 例: h('p', null, phrase('参考値です。簡易課税と特例は反映していません。'))
 * @param {string} text
 * @returns {HTMLElement}
 */
export function phrase(text) {
  return h('span', { class: 'phrase' }, chunks(text).flatMap((c, i) => (i ? [h('wbr'), h('span', { class: 'nw' }, c)] : [h('span', { class: 'nw' }, c)])));
}
