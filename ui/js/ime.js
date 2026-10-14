// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 日本語入力（IME）の決めごとを、ここ1か所に閉じ込める（docs/spec/keyboard.md）。
// 変換を決めるEnterが、候補の選択や保存として働くと、記帳の途中で意図しない仕訳ができてしまう。
// 全画面のキー処理は、最初に shouldIgnoreKey(event) を呼び、真なら何もしない。自分で判定を書かない。

/** compositionend の後、この時間（ミリ秒）以内のEnterは、変換の確定とみなす。経験値で、ブラウザごとの保証は無い。 */
export const IME_ENTER_WINDOW_MS = 50;

/** classifyKey の結果。 */
export const KEY = Object.freeze({
  /** 変換中。何もしない。preventDefault もしない（変換の動きを邪魔しない）。 */
  COMPOSING: 'composing',
  /** 変換の後追い（keyCode 229、または compositionend から50ms以内のEnter）。アプリの処理はせず、preventDefault する。 */
  SWALLOW: 'swallow',
  /** Enterの押しっぱなし。保存・確定候補の処理をしない。 */
  REPEAT: 'repeat',
  /** 通常のキー操作。 */
  NORMAL: 'normal',
});

const state = {
  composing: false,
  lastEndAt: Number.NEGATIVE_INFINITY,
  clock: () => performance.now(),
};

/**
 * 変換の始まりと終わりを、文書全体で見張る。main.js が起動時に1回呼ぶ。
 * @param {EventTarget} [target] 既定は document。試験では EventTarget を渡す。
 * @param {() => number} [clock] 既定は performance.now。試験では差し替える。
 * @returns {() => void} 見張りをやめる関数
 */
export function installImeTracking(target = document, clock = () => performance.now()) {
  state.composing = false;
  state.lastEndAt = Number.NEGATIVE_INFINITY;
  state.clock = clock;
  const onStart = () => {
    state.composing = true;
  };
  const onEnd = () => {
    state.composing = false;
    state.lastEndAt = state.clock();
  };
  // 変換中に別の欄へ移ると、ブラウザによっては compositionend が来ない。状態が残り続けないよう、ここで戻す。
  const onFocusOut = () => {
    state.composing = false;
  };
  // 取り込み側（capture）で受けて、各部品の処理より先に状態を更新する。
  // 真偽値の true ではなく { capture: true } で渡す。Nodeの EventTarget は真偽値の指定では解除できない。
  const opts = { capture: true };
  target.addEventListener('compositionstart', onStart, opts);
  target.addEventListener('compositionend', onEnd, opts);
  target.addEventListener('focusout', onFocusOut, opts);
  return () => {
    target.removeEventListener('compositionstart', onStart, opts);
    target.removeEventListener('compositionend', onEnd, opts);
    target.removeEventListener('focusout', onFocusOut, opts);
    state.composing = false;
  };
}

/** いま変換中なら真（compositionstart から compositionend まで）。 */
export function isComposing() {
  return state.composing;
}

/**
 * キーのイベントを、keyboard.md の順に分ける。
 * 1. event.isComposing（または compositionstart〜end の間）→ COMPOSING
 * 2. keyCode 229、または compositionend から50ms以内のEnter → SWALLOW
 *    （Safariは、変換を決めるEnterの keydown を compositionend の後に isComposing 偽で送る）
 * 3. 押しっぱなしのEnter（event.repeat）→ REPEAT
 * 4. それ以外 → NORMAL
 * @param {{ key?: string, isComposing?: boolean, keyCode?: number, repeat?: boolean }} event
 * @returns {'composing'|'swallow'|'repeat'|'normal'}
 */
export function classifyKey(event) {
  if (!event) return KEY.NORMAL;
  if (event.isComposing || state.composing) return KEY.COMPOSING;
  if (event.keyCode === 229) return KEY.SWALLOW;
  if (event.key === 'Enter' && state.clock() - state.lastEndAt <= IME_ENTER_WINDOW_MS) return KEY.SWALLOW;
  if (event.key === 'Enter' && event.repeat) return KEY.REPEAT;
  return KEY.NORMAL;
}

/**
 * キー処理の入口で呼ぶ。真なら、呼び出し側は何もしないで戻る。AC-05.2、AC-10.4。
 * SWALLOW と REPEAT のときは preventDefault して、フォームの暗黙の送信やボタンの押下が起きないようにする。
 * COMPOSING のときは preventDefault しない（変換そのものを止めてしまうため）。
 * @param {KeyboardEvent} event
 * @returns {boolean}
 */
export function shouldIgnoreKey(event) {
  const kind = classifyKey(event);
  if (kind === KEY.NORMAL) return false;
  if (kind !== KEY.COMPOSING && event && typeof event.preventDefault === 'function') event.preventDefault();
  return true;
}

/**
 * 変換に関わるキーなら真（COMPOSING か SWALLOW）。押しっぱなしは含まない。
 * u1 の依頼書の名前に合わせて残している。新しい処理は shouldIgnoreKey を使う。
 */
export function isImeConfirm(event) {
  const kind = classifyKey(event);
  return kind === KEY.COMPOSING || kind === KEY.SWALLOW;
}
