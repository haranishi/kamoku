// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { initialMapping, keptMapping, dateColumnWarning, columnOptions, columnExamples, isNumberColumn, mappingProblems, describeSource, encodingLabel, delimiterLabel, countsText, fallbackMapping, sampleLines, sampleSkipped, importSummary, summaryText } from '../js/import-map.js';

const upload = {
  encoding: 'shift_jis',
  delimiter: ',',
  header: ['日付', '摘要', '出金', '入金', '残高'],
  sample: [['2026/10/01', 'サンプルストア', '1,650', '', '98,350']],
  guess: { skip_rows: 1, date_col: 0, desc_cols: [1], id_col: null, amount: { mode: 'split', out_col: 2, in_col: 3 } },
  saved: null,
};

test('AC-04.3 列の対応の初期値: 覚えさせた対応 → 推測 → 決まった形 の順', () => {
  assert.equal(initialMapping(upload).source, 'guess');
  const saved = { ...upload.guess, desc_cols: [1, 4] };
  const withSaved = initialMapping({ ...upload, saved });
  assert.equal(withSaved.source, 'saved');
  assert.deepEqual(withSaved.mapping.desc_cols, [1, 4]);
  // 列の数に合わない保存（別の形式のファイル）は使わない。
  assert.equal(initialMapping({ ...upload, saved: { ...saved, date_col: 9 } }).source, 'guess');
  const none = initialMapping({ ...upload, guess: null, header: null });
  assert.equal(none.source, 'none');
  assert.equal(none.mapping.amount.mode, 'signed');
  assert.match(describeSource('none'), /推測できませんでした/);
});

test('SC-04 r3-1 「戻る」で(2)へ戻ったときは、直前に選んだ対応をそのまま使い（写しを返す）、その旨の文を出す', () => {
  const chosen = { ...upload.guess, desc_cols: [1, 4], amount: { mode: 'split', out_col: 4, in_col: 3 } };
  const kept = keptMapping(chosen);
  assert.equal(kept.source, 'kept');
  assert.deepEqual(kept.mapping, chosen);
  // 写しなので、画面で直しても、控えた対応は変わらない。
  kept.mapping.desc_cols.push(0);
  kept.mapping.amount.out_col = 2;
  assert.deepEqual(chosen.desc_cols, [1, 4]);
  assert.equal(chosen.amount.out_col, 4);
  assert.equal(describeSource('kept'), '直前に選んだ列の対応のままです。直したら、もう一度「確かめる」を押してください。');
});

test('AC-04.3 初期値は元の推測を書き換えない（画面で直しても、推測の値は残る）', () => {
  const m = initialMapping(upload).mapping;
  m.desc_cols.push(4);
  m.amount.out_col = 4;
  assert.deepEqual(upload.guess.desc_cols, [1]);
  assert.equal(upload.guess.amount.out_col, 2);
});

// 2026-10-14 の画面の直し（F2）: 選択欄の中で切れないよう、名前は「3列目 出金」と短くし、見本の値は example に分けた。
test('AC-04.3 列の選択肢の名前は「3列目 見出し」。見本の値は分けて持つ', () => {
  const o = columnOptions(upload);
  assert.equal(o.length, 5);
  assert.equal(o[2].label, '3列目 出金');
  assert.equal(o[2].example, '1,650');
  assert.equal(o[3].label, '4列目 入金');
  assert.equal(o[3].example, '');
  const bare = columnOptions({ header: null, sample: [['20261001', 'カフェ', '880']] });
  assert.equal(bare[0].label, '1列目');
  assert.equal(bare[0].example, '20261001');
});

test('AC-04.3 列ごとの見本の値（狭い幅の一覧）と、数の列の見分け（見本の表で右に寄せる）', () => {
  const up = { header: ['日付', '摘要', '出金', '残高'], sample: [['2026/10/01', 'ストア', '1,650', '98350'], ['2026/10/02', 'カフェ', '', '-5,400'], ['2026/10/03', '鉄道', '300', '92950']] };
  assert.deepEqual(columnExamples(up, 2), ['1,650', '300']);
  assert.deepEqual(columnExamples(up, 1, 2), ['ストア', 'カフェ']);
  assert.equal(isNumberColumn(up, 2), true);
  assert.equal(isNumberColumn(up, 3), true);
  assert.equal(isNumberColumn(up, 0), false);
  assert.equal(isNumberColumn(up, 1), false);
  assert.equal(isNumberColumn({ header: null, sample: [['', '']] }, 0), false);
});

test('AC-04.3 誤った対応を見つける（出金と入金が同じ列、日付と金額が同じ列、範囲の外）', () => {
  assert.deepEqual(mappingProblems(upload.guess, 5), []);
  const same = { ...upload.guess, amount: { mode: 'split', out_col: 2, in_col: 2 } };
  assert.ok(mappingProblems(same, 5).some((p) => p.field === 'amount'));
  const clash = { ...upload.guess, amount: { mode: 'signed', col: 0, inflow: 'positive' } };
  assert.ok(mappingProblems(clash, 5).some((p) => p.field === 'date_col'));
  assert.ok(mappingProblems({ ...upload.guess, date_col: 7 }, 5).some((p) => p.field === 'date_col'));
  assert.ok(mappingProblems({ ...upload.guess, skip_rows: -1 }, 5).some((p) => p.field === 'skip_rows'));
});

test('表示名と件数の文', () => {
  assert.equal(encodingLabel('shift_jis'), 'Shift_JIS');
  assert.equal(encodingLabel('utf-8'), 'UTF-8');
  assert.equal(delimiterLabel('\t'), 'タブ');
  assert.equal(countsText({ new: 300, duplicate: 10, invalid: 2 }), '新規 300件・重複 10件・読めない 2件');
  assert.equal(fallbackMapping({ header: null, sample: [['a']] }).date_col, 0);
});

test('SC-04 g2-8 見本の行の番号は、ファイルの行番号（見出しがあれば2から。確かめの画面の「ファイルの○行目」と同じ）', () => {
  assert.deepEqual(sampleLines({ ...upload, sample: [['a'], ['b'], ['c']] }), [2, 3, 4]);
  // 見出しが無い（推測なし）なら1から。注意書きの行と見出しを読み飛ばす推測（2行）なら3から。
  assert.deepEqual(sampleLines({ header: null, guess: null, sample: [['a'], ['b']] }), [1, 2]);
  assert.deepEqual(sampleLines({ ...upload, guess: { ...upload.guess, skip_rows: 2 }, sample: [['a'], ['b']] }), [3, 4]);
  // 引用符の中の改行（1つの欄が2行にまたがる）は、後ろの行の番号を送る。
  assert.deepEqual(sampleLines({ ...upload, sample: [['2026/10/03', '二行の\n摘要'], ['2026/10/04', 'ふつう']] }), [2, 4]);
});

test('SC-04 g2-8 読み飛ばす行の数を推測より増やすと、見本の先頭の行を「読み飛ばす」にする', () => {
  assert.equal(sampleSkipped(upload, 1, 0), false);
  assert.equal(sampleSkipped(upload, 2, 0), true);
  assert.equal(sampleSkipped(upload, 2, 1), false);
  assert.equal(sampleSkipped(upload, Number.NaN, 0), false);
  assert.equal(sampleSkipped({ ...upload, guess: null }, 1, 0), true);
});

test('SC-04 r3-9 日付の列の注意: 取込の確認の応えで、日付として読めない行（理由が「日付」で始まる）が半分を超えたら文を返す', () => {
  const bad = (text) => ({ status: 'invalid', reason: `日付「${text}」を読めません（実在しない日を含む）。` });
  const ok = { status: 'new', reason: null };
  assert.equal(dateColumnWarning([bad('サンプルストア'), bad('デモ鉄道'), { status: 'invalid', reason: '日付が空です。' }]), 'この列は日付に読めない行が多いようです（3行のうち3行）。日付の列を選び直してください。');
  // 半分が読めるなら出さない。日付以外の理由（金額）で読めない行は数えない。重複の行は読めている。
  assert.equal(dateColumnWarning([bad('a'), ok]), '');
  assert.equal(dateColumnWarning([bad('a'), bad('b'), ok]), 'この列は日付に読めない行が多いようです（3行のうち2行）。日付の列を選び直してください。');
  assert.equal(dateColumnWarning([{ status: 'invalid', reason: '金額を数として読めません。' }, { status: 'invalid', reason: '入金と出金の両方に金額があります。' }]), '');
  assert.equal(dateColumnWarning([{ status: 'duplicate', reason: '取り込み済みの明細と同じです。' }]), '');
  assert.equal(dateColumnWarning([]), '');
});

const preview = [
  { line: 2, status: 'new', date: '2026-09-03', direction: 'out', amount: 880 },
  { line: 3, status: 'new', date: '2026-09-01', direction: 'out', amount: 1100 },
  { line: 4, status: 'new', date: '2026-09-30', direction: 'in', amount: 118640 },
  { line: 5, status: 'duplicate', date: '2026-08-31', direction: 'out', amount: 500 },
  { line: 6, status: 'invalid', date: null, direction: null, amount: null },
];

test('SC-04 g2-8 取り込む行の期間と、出金・入金の件数と合計（重複・読めない行は数えない。選んだ重複は数える）', () => {
  const s = importSummary(preview);
  assert.deepEqual(s, { count: 3, from: '2026-09-01', to: '2026-09-30', out: { count: 2, total: 1980 }, in: { count: 1, total: 118640 } });
  assert.equal(summaryText(s), '取り込む3件: 2026-09-01〜2026-09-30・出金 2件 1,980円・入金 1件 118,640円');
  const forced = importSummary(preview, new Set([5]));
  assert.equal(summaryText(forced), '取り込む4件: 2026-08-31〜2026-09-30・出金 3件 2,480円・入金 1件 118,640円');
  // 0件の向きは書かない。1日だけなら、期間は1つの日付。表が先頭の500行だけなら、その旨を書く。
  assert.equal(summaryText(importSummary([preview[0]])), '取り込む1件: 2026-09-03・出金 1件 880円');
  assert.equal(summaryText(importSummary([preview[0]]), true), '表に出している行のうち、取り込む1件: 2026-09-03・出金 1件 880円');
  assert.equal(summaryText(importSummary(preview.slice(3))), '');
});
