// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 全体のキー（keyboard.md の「全体のキー」）。画面ごとの1文字のキー（仕訳帳の j k e v [ ]）も、ここに登録する。
// 文字入力の欄にいるとき（変換中を含む）は、1文字のキーを働かせない（AC-10.4）。

import { shouldIgnoreKey } from './ime.js';

const NON_TEXT_INPUTS = new Set(['checkbox', 'radio', 'button', 'submit', 'reset', 'range', 'color', 'file', 'image']);

/** 文字を打つ欄なら真（input の文字の種類、textarea、select、編集できる要素）。 */
export function isTypingTarget(el) {
  if (!el || typeof el.tagName !== 'string') return false;
  if (el.isContentEditable) return true;
  const tag = el.tagName;
  if (tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (tag === 'INPUT') return !NON_TEXT_INPUTS.has((el.getAttribute('type') || 'text').toLowerCase());
  return false;
}

/** Macなら真（ショートカットの表示を「⌘」にする）。 */
export function isMac() {
  const p = (typeof navigator !== 'undefined' && (navigator.platform || navigator.userAgent)) || '';
  return /Mac|iPhone|iPad/.test(p);
}

/** 「Ctrl」か「⌘」。 */
export function modLabel() {
  return isMac() ? '⌘' : 'Ctrl';
}

const G_TARGETS = { h: '#/', n: '#/entry', i: '#/inbox', j: '#/journal', r: '#/reports', w: '#/wallets', s: '#/settings' };

// 1文字のショートカット（修飾キーを伴わない1文字で、画面全体に働くキー）を切れるようにする（WCAG 2.1.4）。
// 音声入力で話した言葉の1文字や、押し間違いで画面が移ってしまうため。既定は使う。
const SINGLE_KEY = 'kamoku.singleKeys';
let single = null;
const singleListeners = new Set();

/** 1文字のショートカットを使うなら真（既定）。保存した設定を読めなければ、使う。 */
export function singleKeysEnabled() {
  if (single === null) {
    try {
      single = localStorage.getItem(SINGLE_KEY) !== 'off';
    } catch {
      single = true;
    }
  }
  return single;
}

/** 1文字のショートカットを使うか切るかを決めて、覚えておく。保存できなくても、この画面を開いている間は効く。 */
export function setSingleKeys(on) {
  single = Boolean(on);
  try {
    localStorage.setItem(SINGLE_KEY, single ? 'on' : 'off');
  } catch {
    // 保存できない（プライベートな窓など）。
  }
  for (const fn of singleListeners) fn(single);
}

/** 切り替わったら fn(使うか) を呼ぶ。止める関数を返す。 */
export function onSingleKeysChange(fn) {
  singleListeners.add(fn);
  return () => singleListeners.delete(fn);
}

/** 文字のキー（1文字の e.key）。Esc・Enter・Tab・矢印は名前が長いので当たらない。 */
const isCharKey = (e) => typeof e.key === 'string' && e.key.length === 1;

/**
 * 文書全体のキー操作を見張る。main.js が1回だけ作る。
 * @param {{ navigate: (hash: string) => void, openPalette: () => void, togglePalette?: () => void, openHelp: () => void, focusSearch: () => boolean }} actions
 */
export function createKeys(actions) {
  let gTimer = null;
  const screenMaps = [];

  function clearG() {
    if (gTimer) clearTimeout(gTimer);
    gTimer = null;
  }

  function onKeyDown(e) {
    if (shouldIgnoreKey(e)) return;
    if (e.defaultPrevented) return;
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.altKey && !e.shiftKey && (e.key === 'k' || e.key === 'K')) {
      e.preventDefault();
      clearG();
      (actions.togglePalette || actions.openPalette)();
      return;
    }
    // ダイアログや引き出しが開いている間は、その中のキー処理に任せる。
    if (document.querySelector('dialog[open]')) return;
    if (e.altKey || mod) return;
    if (isTypingTarget(e.target)) {
      clearG();
      return;
    }
    // 切ってあるときは、g＋文字・画面の1文字のキー・n / ? を働かせず、ブラウザの既定の動き（空白でのスクロールなど）に任せる。
    if (isCharKey(e) && !singleKeysEnabled()) {
      clearG();
      return;
    }
    if (gTimer) {
      clearG();
      const hash = G_TARGETS[e.key];
      if (hash) {
        e.preventDefault();
        actions.navigate(hash);
      }
      return;
    }
    if (e.key === 'g') {
      gTimer = setTimeout(clearG, 1500);
      return;
    }
    for (let i = screenMaps.length - 1; i >= 0; i -= 1) {
      const fn = screenMaps[i][e.key];
      if (fn && fn(e) !== false) {
        e.preventDefault();
        return;
      }
    }
    if (e.key === 'n') {
      e.preventDefault();
      actions.navigate('#/entry');
    } else if (e.key === '/') {
      e.preventDefault();
      if (!actions.focusSearch()) actions.openPalette();
    } else if (e.key === '?') {
      e.preventDefault();
      actions.openHelp();
    }
  }

  document.addEventListener('keydown', onKeyDown);

  return {
    /**
     * 画面の1文字のキーを登録する。例: keys.register({ j: next, k: prev })。
     * 関数が false を返すと、そのキーは処理しなかったものとして、全体のキーに回す。片付けの関数を返す。
     */
    register(map) {
      screenMaps.push(map);
      return () => {
        const i = screenMaps.indexOf(map);
        if (i >= 0) screenMaps.splice(i, 1);
      };
    },
    destroy() {
      document.removeEventListener('keydown', onKeyDown);
      clearG();
    },
  };
}
