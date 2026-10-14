// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 2つの検査（色のコントラスト、ソースの決まり）が、違反を確かに見つけるか。門番が眠っていないことを確かめる。

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { contrast, checkTokens, parseTokens } from '../dev/check-contrast.mjs';
import { checkSource } from '../dev/check-policy.mjs';

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

const has = (rel, text, needle) => checkSource(rel, text).some((p) => p.message.includes(needle));

test('決まり3: innerHTML・eval・new Function を見つける', () => {
  assert.ok(has('js/screens/x.js', 'el.innerHTML = s;', 'innerHTML'));
  assert.ok(has('js/screens/x.js', 'const v = eval(text);', 'eval'));
  assert.ok(has('js/screens/x.js', 'new Function("return 1")', 'new Function'));
  assert.ok(has('js/screens/x.js', "el.insertAdjacentHTML('beforeend', s)", 'insertAdjacentHTML'));
  // 説明のコメントは対象外。
  assert.deepEqual(checkSource('js/x.js', '// innerHTML は使わない'), []);
});

test('決まり2: style 属性・インラインのスクリプトとイベントを見つける', () => {
  assert.ok(has('js/x.js', "el.setAttribute('style', 'width: 1px')", 'setAttribute'));
  assert.ok(has('index.html', '<script>alert(1)</script>', 'インラインの <script>'));
  assert.ok(has('index.html', '<div style="color: red">', 'style 属性'));
  assert.ok(has('index.html', '<button onclick="go()">', 'インラインのイベント'));
  assert.deepEqual(checkSource('index.html', '<script type="module" src="js/main.js"></script>'), []);
});

test('決まり4: 外のURLと、api.js 以外の fetch を見つける。SVGの名前空間だけは許す', () => {
  assert.ok(has('js/x.js', "const u = 'https://example.com/a.js';", '外のURL'));
  assert.ok(has('css/x.css', "@import url('https://fonts.example/f.css');", '外のURL'));
  assert.ok(has('js/screens/x.js', "fetch('/api/v1/status')", 'fetch'));
  assert.deepEqual(checkSource('js/api.js', "res = await fetch(url, init);"), []);
  assert.deepEqual(checkSource('js/dom.js', "export const SVG_NS = 'http://www.w3.org/2000/svg';"), []);
});

test('決まり5: 部品のCSSの16進の色と、色の関数を見つける（tokens.css は対象外）', () => {
  assert.ok(has('css/components.css', '.x { color: #FF0000; }', '16進の色'));
  assert.ok(has('css/components.css', '.x { color: rgb(255 0 0); }', '色の関数'));
  assert.deepEqual(checkSource('css/tokens.css', ':root { --c-bg: #F6F5F1; }'), []);
  assert.deepEqual(checkSource('css/components.css', '.x { color: var(--c-text); }'), []);
});

test('決まり6: 絵文字のアイコンと、ime.js の外でのIMEの判定を見つける', () => {
  assert.ok(has('js/x.js', "h('span', null, '📊')", '絵文字'));
  assert.ok(has('js/screens/x.js', 'if (e.isComposing) return;', 'IMEの判定'));
  assert.ok(has('js/screens/x.js', 'if (e.keyCode === 229) return;', 'IMEの判定'));
  assert.deepEqual(checkSource('js/ime.js', 'if (event.isComposing) return true;'), []);
});
