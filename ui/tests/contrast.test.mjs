// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 2つの検査（色のコントラスト、ソースの決まり）が、違反を確かに見つけるか。門番が眠っていないことを確かめる。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { contrast, checkTokens, parseTokens } from '../dev/check-contrast.mjs';

const tokens = readFileSync(new URL('../css/tokens.css', import.meta.url), 'utf8');

test('AC-11.2 コントラスト比の計算（白と黒は21:1、同じ色は1:1）', () => {
  assert.equal(Math.round(contrast('#000000', '#FFFFFF') * 100) / 100, 21);
  assert.equal(contrast('#777', '#777'), 1);
  // #767676 は白の上で4.54:1（基準ぎりぎりで通る）、#777777 は4.48:1（通らない）。
  assert.ok(contrast('#767676', '#FFFFFF') >= 4.5);
  assert.ok(contrast('#777777', '#FFFFFF') < 4.5);
});

test('AC-11.2 いまの tokens.css は、ライトとダークの両方で基準を満たす', () => {
  const { rows, problems } = checkTokens(tokens);
  assert.deepEqual(problems, []);
  assert.ok(rows.length >= 60);
});

test('AC-11.2 文字の色を薄くすると、検査が落ちる', () => {
  const bad = tokens.replace('--c-muted: #566170;', '--c-muted: #9AA3AE;');
  const { problems } = checkTokens(bad);
  assert.ok(problems.some((p) => p.includes('--c-muted')));
});

test('AC-11.2 入力欄の枠が背景に溶けると、検査が落ちる（3:1）', () => {
  const bad = tokens.replace('--c-border-strong: #8A8576;', '--c-border-strong: #C9C5BB;');
  assert.ok(checkTokens(bad).problems.some((p) => p.includes('--c-border-strong')));
});

test('OSに従うダークと手動のダークで値が食い違うと、検査が落ちる', () => {
  const t = parseTokens(tokens);
  assert.equal(t.darkOs['--c-text'], t.darkManual['--c-text']);
  const idx = tokens.lastIndexOf('--c-text: #E8ECF1;');
  const bad = `${tokens.slice(0, idx)}--c-text: #D0D4D9;${tokens.slice(idx + '--c-text: #E8ECF1;'.length)}`;
  assert.ok(checkTokens(bad).problems.some((p) => p.includes('--c-text') && p.includes('違います')));
});
