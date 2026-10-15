// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 全体のキー（js/keys.js）。1文字のショートカットを切る設定（WCAG 2.1.4）を、偽の document と localStorage で確かめる。

import { test } from 'node:test';
import assert from 'node:assert/strict';

let fresh = 0;
/** 設定の覚え（モジュールの中の値）を持ち越さないよう、毎回新しく読み込む。 */
const loadKeys = () => import(`../js/keys.js?fresh=${(fresh += 1)}`);

const saved = new WeakMap();
/** 全体の値を差し替える。試験の終わりに、試験の前の値へ戻す（同じ名前を何度差し替えても、最初の値に戻す）。 */
function put(t, key, value) {
  if (!saved.has(t)) {
    const before = new Map();
    saved.set(t, before);
    t.after(() => {
      for (const [k, d] of before) {
        if (d) Object.defineProperty(globalThis, k, d);
        else delete globalThis[k];
      }
    });
  }
  const before = saved.get(t);
  if (!before.has(key)) before.set(key, Object.getOwnPropertyDescriptor(globalThis, key));
  Object.defineProperty(globalThis, key, { configurable: true, writable: true, value });
}

/** 文書（キーの見張りと、開いている小窓の有無）と、保存の場所。 */
function environment(t, storage = new Map()) {
  const listeners = new Map();
  put(t, 'document', {
    addEventListener: (type, fn) => listeners.set(type, fn),
    removeEventListener: (type) => listeners.delete(type),
    querySelector: () => null,
  });
  put(t, 'localStorage', {
    getItem: (k) => (storage.has(k) ? storage.get(k) : null),
    setItem: (k, v) => storage.set(k, String(v)),
  });
  const calls = [];
  const actions = {
    navigate: (hash) => calls.push(['navigate', hash]),
    openPalette: () => calls.push(['palette']),
    togglePalette: () => calls.push(['toggle-palette']),
    openHelp: () => calls.push(['help']),
    focusSearch: () => {
      calls.push(['search']);
      return true;
    },
  };
  const BUTTON = { tagName: 'BUTTON', getAttribute: () => null };
  /** キーを1つ押す。preventDefault されたかを返す。 */
  const press = (key, mods = {}) => {
    const e = { key, target: BUTTON, ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, isComposing: false, keyCode: 0, repeat: false, defaultPrevented: false, ...mods };
    e.preventDefault = () => {
      e.defaultPrevented = true;
    };
    listeners.get('keydown')(e);
    return e.defaultPrevented;
  };
  return { storage, calls, actions, press };
}

test('AC-10.5 既定は1文字のキーが働き、切ると n・?・/・g＋文字・画面の1文字のキーを止める。⌘K と Esc は働く。戻すとまた働く', async (t) => {
  const env = environment(t);
  const keys = await loadKeys();
  assert.equal(keys.singleKeysEnabled(), true, '既定は使う');
  const k = keys.createKeys(env.actions);
  const screen = [];
  k.register({ j: () => screen.push('j'), Escape: () => screen.push('Escape') });

  assert.equal(env.press('n'), true);
  env.press('g');
  env.press('w');
  env.press('?');
  env.press('/');
  env.press('j');
  assert.deepEqual(env.calls, [['navigate', '#/entry'], ['navigate', '#/wallets'], ['help'], ['search']]);
  assert.deepEqual(screen, ['j']);

  const seen = [];
  const stop = keys.onSingleKeysChange((on) => seen.push(on));
  keys.setSingleKeys(false);
  assert.equal(keys.singleKeysEnabled(), false);
  assert.equal(env.storage.get('kamoku.singleKeys'), 'off', '次に開いたときのために覚える');
  assert.deepEqual(seen, [false]);

  env.calls.length = 0;
  screen.length = 0;
  // 止めたキーは preventDefault もしない（空白でのスクロールなど、ブラウザの既定の動きに任せる）。
  assert.equal(env.press('n'), false);
  env.press('g');
  env.press('h');
  env.press('?');
  env.press('/');
  env.press('j');
  env.press('N', { shiftKey: true });
  assert.deepEqual(env.calls, [], '1文字のキーは、どれも働かない');
  assert.deepEqual(screen, [], '画面の1文字のキー（仕訳帳の j など）も働かない');
  // 組み合わせのキーと、文字でないキーは働く。
  env.press('k', { metaKey: true });
  env.press('Escape');
  assert.deepEqual(env.calls, [['toggle-palette']]);
  assert.deepEqual(screen, ['Escape']);

  keys.setSingleKeys(true);
  assert.equal(env.storage.get('kamoku.singleKeys'), 'on');
  env.calls.length = 0;
  env.press('n');
  assert.deepEqual(env.calls, [['navigate', '#/entry']]);
  assert.deepEqual(seen, [false, true]);
  stop();
  keys.setSingleKeys(false);
  assert.deepEqual(seen, [false, true], '止めた後は呼ばない');
  k.destroy();
});

test('AC-10.5 切った設定は、開き直しても残る。保存の場所が使えなくても、画面は動く（既定は使う）', async (t) => {
  const stored = environment(t, new Map([['kamoku.singleKeys', 'off']]));
  let keys = await loadKeys();
  assert.equal(keys.singleKeysEnabled(), false, '覚えた「切る」を読む');
  const k = keys.createKeys(stored.actions);
  stored.press('n');
  assert.deepEqual(stored.calls, []);
  k.destroy();

  // 読み書きで例外が出る（プライベートな窓など）。
  put(t, 'localStorage', {
    getItem: () => {
      throw new Error('使えません');
    },
    setItem: () => {
      throw new Error('使えません');
    },
  });
  keys = await loadKeys();
  assert.equal(keys.singleKeysEnabled(), true);
  assert.doesNotThrow(() => keys.setSingleKeys(false));
  assert.equal(keys.singleKeysEnabled(), false, '保存できなくても、この画面の間は効く');

  // localStorage そのものが無い。
  delete globalThis.localStorage;
  keys = await loadKeys();
  assert.equal(keys.singleKeysEnabled(), true);
});
