// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 日本語の文を、語の途中で折らない塊に分ける（js/components/phrase.js）。帳票の注意の帯・注記・数字の見出しで使う。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { chunks, phrase } from '../js/components/phrase.js';

const DISCLAIMER = '参考値です。簡易課税と特例、経過措置の適用条件（請求書等と帳簿の保存、一の仕入先ごとの上限）は反映していません。課税の区分は現在の設定を使っています。経過措置の表の最終確認日は2026-10-14です。';

test('SC-06 塊をつなぐと元の文に戻り、どの塊も14字以内', () => {
  for (const t of [DISCLAIMER, '参考値。簡易課税・特例は反映していません', '仕入税額（経過措置の控除を含む）', '']) {
    const c = chunks(t);
    assert.equal(c.join(''), t);
    assert.ok(c.every((x) => x.length <= 14), c.join(' | '));
  }
});

test('SC-06 句読点のあと・開き括弧の前・ひらがなから漢字に変わる所で切り、語の途中では切らない', () => {
  const c = chunks(DISCLAIMER);
  // 「一」が行末に1字だけ残らない（「一の」で1つの塊）。「反映していません。」は割らない。
  assert.ok(c.includes('一の'), c.join(' | '));
  assert.ok(c.includes('反映していません。'));
  assert.ok(c.includes('参考値です。'));
  // 閉じ括弧のあとに助詞が続くときは、結ぶ（行頭に「は」を置かない）。
  assert.ok(c.includes('上限）は'));
  // 開き括弧の前で切る。
  assert.ok(c.includes('（請求書等と'));
  // 日付と「です。」は1つの塊。
  assert.equal(c.at(-1), '2026-10-14です。');
  assert.deepEqual(chunks('参考値。簡易課税・特例は反映していません'), ['参考値。', '簡易課税・特例は', '反映していません']);
});

test('SC-06 「その」「この」だけを行末に残さない（次の塊と結ぶ）', () => {
  assert.deepEqual(chunks('事業主貸は、その年の動きだけです。'), ['事業主貸は、', 'その年の', '動きだけです。']);
  assert.ok(chunks('この期間は、ありません').includes('この期間は、'));
  assert.deepEqual(chunks('超えた分にはこの特例を使えない'), ['超えた', '分には', 'この特例を', '使えない']);
});

test('SC-06 送り仮名（漢字＋ひらがな1字＋漢字）では切らない。助詞の1字では切ってよい', () => {
  assert.ok(chunks('税額を差し引ける、期間を区切った特例').includes('差し引ける、'));
  assert.ok(chunks('取消の手続きで取り消すと、書き込みます').includes('取り消すと、'));
  assert.deepEqual(chunks('帳簿の保存'), ['帳簿の', '保存']);
  assert.deepEqual(chunks('「現金」口座'), ['「現金」', '口座']);
  assert.deepEqual(chunks('「現金」の口座'), ['「現金」の', '口座']);
});

// 採点で「語の途中で折れた」と指摘された文が、その語を1つのかたまりに持つこと（かたまりの中では折り返さない）。
const CASES = [
  ['決算や申告が済んだ期間を締めると、その期間の登録・訂正・取消を断ります。空にすると締めません。', ['締めません。', '断ります。', '締めると、']],
  ['ふつうは開業日（または、記帳を始める年の1月1日）です。最初の会計年度の中の日付にしてください。', ['1月1日）です。', '日付にしてください。', '開業日（または、']],
  ['金融機関が付けた番号の列があれば選んでください。重複を見分ける、いちばん確かな手がかりです。', ['選んでください。', '手がかりです。']],
  ['開業日の時点で、それぞれの口座にいくらあったかを入れます。カードは、まだ払っていない額です。', ['口座にいくらあったかを', '払っていない額です。']],
  ['型は「現金」「預金」「クレジットカード」「電子マネー」から選び、名前は自由に付けられます。', ['「電子マネー」から', '「クレジットカード」']],
  ['2026年1月1日から9月30日までの集計です。参考値で、簡易課税と特例は反映していません。', ['2026年1月1日から', '9月30日までの', '簡易課税と', '反映していません。']],
  ['負債・カードの未払い額', ['負債・カードの', '未払い額']],
  // 送り仮名の後では区切らない（「引き／落とし」と割れた）。
  ['サンプルカードの引き落とし', ['引き落とし']],
  ['確定すると、仕訳帳と帳票の集計に入ります。確定した後は、訂正と取消の手続きでしか変えられません。', ['確定すると、', '変えられません。']],
];

test('SC-10 補足文を、語の途中で割らないかたまりに分ける（つなげると元の文）', () => {
  for (const [text, whole] of CASES) {
    const parts = chunks(text);
    assert.equal(parts.join(''), text);
    assert.ok(parts.every((c) => [...c].length <= 14), parts.join('｜'));
    for (const w of whole) assert.ok(parts.includes(w), `「${w}」が1つのかたまりになっていない: ${parts.join('｜')}`);
    // 行の頭に来るかたまりが、閉じかっこ・句読点・ひらがなの「ー」で始まらない。
    for (const c of parts.slice(1)) assert.doesNotMatch(c, /^[、。）」』ー]/, `「${c}」から行が始まりうる: ${parts.join('｜')}`);
  }
});

test('SC-10 長すぎるかたまりは14文字で区切る。空白の後でも区切る。空の文は空', () => {
  const long = 'あ'.repeat(5) + 'ア'.repeat(20);
  assert.ok(chunks(long).every((c) => [...c].length <= 14));
  assert.deepEqual(chunks('UTF-8 と Shift_JIS'), ['UTF-8 ', 'と ', 'Shift_JIS']);
  assert.deepEqual(chunks(''), []);
});

test('phrase は単一spanを返し、塊の間だけにwbrを置く', (t) => {
  const before = Object.getOwnPropertyDescriptor(globalThis, 'document');
  t.after(() => {
    if (before) Object.defineProperty(globalThis, 'document', before);
    else delete globalThis.document;
  });
  globalThis.document = {
    createElement(tag) {
      return { tag, attrs: {}, children: [], setAttribute(k, v) { this.attrs[k] = v; }, append(v) { this.children.push(v); } };
    },
  };
  const node = phrase('帳簿の保存');
  assert.equal(node.tag, 'span');
  assert.equal(node.attrs.class, 'phrase');
  assert.deepEqual(node.children.map((c) => c.tag), ['span', 'wbr', 'span']);
  assert.deepEqual(node.children.filter((c) => c.tag === 'span').map((c) => [c.attrs.class, c.children.join('')]), [['nw', '帳簿の'], ['nw', '保存']]);
  assert.deepEqual(phrase('').children, []);
});

test('SC-04 英数字の語（Shift_JIS・UTF-8）は、長さの上限で切るときも真ん中で割らない。塊の全体が語なら、上限で切る', () => {
  const t = 'ドラッグして置くか、「ファイルを選ぶ」から選んでください。文字コード（UTF-8・Shift_JIS）は自動で判定します。10MBまで。';
  const c = chunks(t);
  assert.equal(c.join(''), t);
  assert.ok(c.every((x) => x.length <= 14), c.join(' | '));
  assert.ok(c.some((x) => x.includes('Shift_JIS')), c.join(' | '));
  assert.ok(c.some((x) => x.includes('UTF-8')), c.join(' | '));
  assert.deepEqual(chunks('abcdefghijklmnopqrstuvwxyz0123'), ['abcdefghijklmn', 'opqrstuvwxyz01', '23']);
});
