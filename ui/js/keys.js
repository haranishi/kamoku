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
