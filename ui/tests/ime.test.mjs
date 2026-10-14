// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  installImeTracking,
  classifyKey,
  shouldIgnoreKey,
  isImeConfirm,
  isComposing,
  KEY,
  IME_ENTER_WINDOW_MS,
} from '../js/ime.js';

// 偽の時計と、Node標準の EventTarget で、ブラウザ無しに決めごと（keyboard.md）を確かめる。
function setup() {
  let now = 1000;
  const target = new EventTarget();
  const uninstall = installImeTracking(target, () => now);
  return {
    uninstall,
    at(t) {
      now = t;
    },
    fire(type) {
      target.dispatchEvent(new Event(type));
    },
  };
}

// preventDefault が呼ばれたかを数える偽のキーイベント。
function key(props) {
  const ev = { key: 'Enter', isComposing: false, keyCode: 13, repeat: false, prevented: 0, ...props };
  ev.preventDefault = () => {
    ev.prevented += 1;
  };
  return ev;
}

test('AC-05.2 (a) event.isComposing が真なら何もしない。preventDefault もしない', () => {
  const t = setup();
  const ev = key({ isComposing: true });
  assert.equal(classifyKey(ev), KEY.COMPOSING);
  assert.equal(shouldIgnoreKey(ev), true);
  assert.equal(ev.prevented, 0, '変換中は preventDefault してはいけない');
  t.uninstall();
});

test('AC-05.2 (a) compositionstart から compositionend の間も、変換中として扱う', () => {
  const t = setup();
  t.fire('compositionstart');
  assert.equal(isComposing(), true);
  const ev = key({ key: 'ArrowDown', keyCode: 40 });
  assert.equal(classifyKey(ev), KEY.COMPOSING);
  assert.equal(shouldIgnoreKey(ev), true);
  assert.equal(ev.prevented, 0);
  t.at(2000);
  t.fire('compositionend');
  assert.equal(isComposing(), false);
  t.uninstall();
});

test('AC-05.2 (b) keyCode 229 は、アプリの処理をせず preventDefault する', () => {
  const t = setup();
  const ev = key({ key: 'Process', keyCode: 229 });
  assert.equal(classifyKey(ev), KEY.SWALLOW);
  assert.equal(shouldIgnoreKey(ev), true);
  assert.equal(ev.prevented, 1);
  t.uninstall();
});

test('AC-05.2 (b) compositionend から50ms以内のEnterは、変換の確定として捨て、preventDefault する（Safari）', () => {
  const t = setup();
  t.fire('compositionstart');
  t.at(5000);
  t.fire('compositionend');
  t.at(5000 + 10);
  const early = key({});
  assert.equal(classifyKey(early), KEY.SWALLOW);
  assert.equal(shouldIgnoreKey(early), true);
  assert.equal(early.prevented, 1);
  t.at(5000 + IME_ENTER_WINDOW_MS);
  assert.equal(classifyKey(key({})), KEY.SWALLOW);
  // Enter以外のキーは、時間の窓の対象外。
  t.at(5000 + 10);
  assert.equal(classifyKey(key({ key: 'ArrowDown', keyCode: 40 })), KEY.NORMAL);
  t.at(5000 + IME_ENTER_WINDOW_MS + 1);
  const later = key({});
  assert.equal(classifyKey(later), KEY.NORMAL);
  assert.equal(shouldIgnoreKey(later), false);
  assert.equal(later.prevented, 0);
  t.uninstall();
});

test('AC-05.2 (c) 押しっぱなしのEnter（repeat）は、保存や確定候補の処理をしない', () => {
  const t = setup();
  const held = key({ repeat: true });
  assert.equal(classifyKey(held), KEY.REPEAT);
  assert.equal(shouldIgnoreKey(held), true);
  assert.equal(held.prevented, 1);
  // 押しっぱなしでも、Enter以外（矢印など）は通常どおり。
  assert.equal(classifyKey(key({ key: 'ArrowDown', keyCode: 40, repeat: true })), KEY.NORMAL);
  t.uninstall();
});

test('AC-05.2 判定の順序: 変換中は、keyCode 229 や repeat より先に (a) になる', () => {
  const t = setup();
  assert.equal(classifyKey(key({ isComposing: true, keyCode: 229, repeat: true })), KEY.COMPOSING);
  t.at(9000);
  t.fire('compositionend');
  t.at(9001);
  assert.equal(classifyKey(key({ repeat: true })), KEY.SWALLOW);
  t.uninstall();
});

test('AC-10.4 変換していないときの文字キーは、通常のキー', () => {
  const t = setup();
  const n = key({ key: 'n', keyCode: 78 });
  assert.equal(classifyKey(n), KEY.NORMAL);
  assert.equal(shouldIgnoreKey(n), false);
  assert.equal(isImeConfirm(n), false);
  assert.equal(classifyKey(null), KEY.NORMAL);
  t.uninstall();
});

test('変換中に欄を離れたら（focusout）、変換中の状態を残さない', () => {
  const t = setup();
  t.fire('compositionstart');
  t.fire('focusout');
  assert.equal(isComposing(), false);
  t.uninstall();
});

test('見張りをやめた後は、イベントを受けても状態が変わらない', () => {
  const t = setup();
  t.uninstall();
  t.fire('compositionstart');
  assert.equal(isComposing(), false);
  assert.equal(classifyKey(key({})), KEY.NORMAL);
});
