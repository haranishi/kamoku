// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 通知。入れ物は aria-live="polite"（index.html の #toasts）。フォーカスは奪わない。
// 成功は4秒、操作つき（「仕訳を見る」など）は10秒で消える（読み上げの途中で消えないよう、マウスやフォーカスが乗っている間は待つ）。
// エラーは role="alert" で、利用者が閉じるまで消さない（screens-other.md の共通の部品）。
// 入れ物はポップオーバー（popover="manual"）で、出すたびに最前面へ出し直す。ただし、モーダルの <dialog> が開いている間は、
// その外の要素がすべて不活性になり、最前面に描いても押せない。そこで、入れ物を最前面の小窓の中へ移してから出す。
// 置き場所は toast-place.js の候補から選ぶ（上を先に試し、フォーカスのある欄に重なるなら下へ。小窓のときは外か見出しの下）。

import { h } from '../dom.js';
import { icon } from '../icons.js';
import { iconButton } from './fields.js';
import { createPlacement } from './toast-place.js';

const SUCCESS_MS = 4000;
const ACTION_MS = 10000;
const MAX_VISIBLE = 4;
let region = null;
let home = null;
let stopTracking = null;
// 置き場所（toast-place.js）。choose が真なら、次に置くときに候補を選び直す（出したとき・フォーカスが動いたとき）。
const placement = createPlacement();
let choose = true;
// 通知の要素 → { stop: 時計を止める, dismiss: 閉じる, text: エラーの文, key: 同じ種類の印 }
const live = new WeakMap();

function updateLift() {
  let lift = 0;
  if (!document.querySelector('dialog[open]')) {
    for (const bar of document.querySelectorAll('[data-bottom-bar]')) {
      const r = bar.getBoundingClientRect();
      const css = getComputedStyle(bar);
      const inset = Number.parseFloat(css.bottom);
      // 通常配置の帯や、下端に付いていない帯では通知を持ち上げない。
      if (!['sticky', 'fixed'].includes(css.position) || !Number.isFinite(inset)) continue;
      if (r.height <= 0 || r.width <= 0 || r.top >= window.innerHeight || r.bottom <= 0 || css.visibility === 'hidden') continue;
      if (Math.abs(r.bottom - (window.innerHeight - inset)) > 2) continue;
      const baseInset = window.matchMedia('(max-width: 767px)').matches ? inset : 0;
      lift = Math.max(lift, window.innerHeight - r.top - baseInset);
    }
  }
  const value = `${Math.ceil(lift)}px`;
  if (region.style.getPropertyValue('--toast-lift') !== value) region.style.setProperty('--toast-lift', value);
}

function isModal(dialog) {
  try {
    return dialog.matches(':modal');
  } catch {
    return true; // :modal を知らないブラウザ。この画面の小窓は、どれも showModal() で開く。
  }
}

/** 入れ物を置く場所。開いているモーダルの小窓のうち、あとから開いたもの（小窓は body の末尾に足すので、文書の順で最後）。 */
function hostNow() {
  const open = [...document.querySelectorAll('dialog[open]')].filter(isModal);
  return open.length ? open[open.length - 1] : home;
}

/** 最前面へ出し直す（あとから開いた小窓より上に来るように）。ポップオーバーの無いブラウザでは、今までどおり。 */
function raise() {
  if (typeof region.showPopover !== 'function' || !region.isConnected) return;
  // 通知のボタンにフォーカスがあるときは出し直さない（閉じると、フォーカスが前の場所へ戻ることがある）。
  if (region.contains(document.activeElement)) return;
  try {
    if (region.matches(':popover-open')) region.hidePopover();
    if (region.children.length) region.showPopover();
  } catch {
    // 出せないとき（文書から外れた直後など）も、通知そのものは今までどおり描かれる。
  }
}

/** 押せる場所へ移す。移したら（移すと閉じるので）出し直す。force なら、移さなくても出し直す。 */
function place(force = false) {
  const host = hostNow();
  const moved = Boolean(host) && region.parentNode !== host;
  if (moved) host.append(region);
  if (moved || force) raise();
}

/** 通知が無くなったら、最前面から下ろし、元の場所へ戻す（小窓と一緒に消えないように）。 */
function settle() {
  try {
    if (typeof region.hidePopover === 'function' && region.isConnected && region.matches(':popover-open')) region.hidePopover();
  } catch {
    // 下ろせなくても、空の入れ物は何も描かない。
  }
  if (home && region.parentNode !== home) home.append(region);
}

function trackBars() {
  if (!region.children.length) {
    if (stopTracking) stopTracking();
    stopTracking = null;
    region.style.setProperty('--toast-lift', '0px');
    placement.reset(region);
    choose = true;
    settle();
    return;
  }
  updateLift();
  placement.place(region, hostNow(), home, choose);
  choose = false;
  if (stopTracking) return;
  let frame = null;
  const schedule = () => {
    if (frame !== null) return;
    frame = requestAnimationFrame(() => {
      frame = null;
      trackBars();
    });
  };
  const sizes = new ResizeObserver(schedule);
  const observeBars = () => {
    sizes.disconnect();
    for (const bar of document.querySelectorAll('[data-bottom-bar]')) sizes.observe(bar);
  };
  // 画面の移動・小窓・表示切替は、寸法が変わらないこともある。小窓が開いたり閉じたりしたら、入れ物もついて行く
  // （閉じる知らせより先に、属性の変化が届くので、小窓が文書から外される前に移せる）。
  const changes = new MutationObserver(() => {
    if (region.children.length) place();
    observeBars();
    schedule();
  });
  // フォーカスが動いたら、置き場所を選び直す（移った先の欄が通知の下に来たら、通知のほうをどける）。
  const onFocus = (e) => {
    if (region.contains(e.target)) return;
    choose = true;
    schedule();
  };
  // 画面の大きさが変わると、フォーカスのある欄が動き、通知の下に入ることがある。大きさの変化は、まれで連続しないので、選び直す
  // （スクロールでは選び直さない。読んでいる通知が上下に跳ねないように）。
  const onResize = () => {
    choose = true;
    schedule();
  };
  observeBars();
  changes.observe(document.body, { childList: true, subtree: true, attributes: true, attributeFilter: ['hidden', 'open', 'class'] });
  window.addEventListener('resize', onResize);
  document.addEventListener('scroll', schedule, true);
  document.addEventListener('focusin', onFocus, true);
  stopTracking = () => {
    sizes.disconnect();
    changes.disconnect();
    window.removeEventListener('resize', onResize);
    document.removeEventListener('scroll', schedule, true);
    document.removeEventListener('focusin', onFocus, true);
    if (frame !== null) cancelAnimationFrame(frame);
  };
}

/** 入れ物を決める。main.js が起動時に呼ぶ。 */
export function initToasts(el) {
  if (stopTracking) stopTracking();
  stopTracking = null;
  region = el;
  home = el ? el.parentNode : null;
  trackBars();
}

function actionEl(action, dismiss) {
  if (!action) return null;
  if (action.href) {
    return h('a', { href: action.href, class: 'link-btn', onclick: () => dismiss() }, action.label);
  }
  return h(
    'button',
    {
      type: 'button',
      class: 'link-btn',
      onclick: () => {
        dismiss();
        action.onClick();
      },
    },
    action.label,
  );
}

/**
 * 同じ種類の短い知らせ（操作もくわしい文も無い、エラーでない知らせ）の印。「」の中の言葉と数字だけが違う文は同じ種類とみなし、
 * 新しい知らせで前の知らせを置き換える（表示の切り替えを続けて押すと、読み上げの領域に前の文が積み重なったため）。
 */
function sameKind(kind, message, action, detail) {
  if (kind === 'error' || action || detail) return null;
  return `${kind}:${String(message).replace(/「[^」]*」/g, '「」').replace(/\d[\d,]*/g, '0')}`;
}

function drop(el) {
  const t = live.get(el);
  if (t) t.stop();
  el.remove();
}

function show(kind, message, { action, detail, ms, replaceKey } = {}) {
  if (!region) return () => {};
  // 同じ文のエラーは重ねない。出ている通知を末尾へ出し直して、もう一度読み上げる（押した手応えを返す）。
  const text = kind === 'error' ? `${message}\n${detail || ''}` : null;
  const same = text && [...region.children].find((x) => live.get(x) && live.get(x).text === text);
  if (same) {
    region.append(same);
    place(true);
    choose = true;
    trackBars();
    return live.get(same).dismiss;
  }
  let timer = null;
  let hovered = false;
  // replaceKey があれば、同じ印の前の通知を置き換える（操作つきの通知は、ふつう置き換えない。続けて無視したとき、右下に積み重なって表を覆った）。
  const key = replaceKey ? `${kind}:key:${replaceKey}` : sameKind(kind, message, action, detail);
  // 入れ物が aria-live="polite" なので、成功には役割を付けない（二重に読まれないように）。エラーだけ割り込みで読む。
  const item = h('div', { class: ['toast', `toast--${kind}`], role: kind === 'error' ? 'alert' : null });
  const stop = () => clearTimeout(timer);
  const dismiss = () => {
    stop();
    item.remove();
    trackBars();
  };
  live.set(item, { stop, dismiss, text, key });
  item.append(
    icon(kind === 'error' ? 'alert' : kind === 'success' ? 'check-circle' : 'info', { className: 'toast-icon' }),
    h(
      'div',
      { class: 'toast-body' },
      h('p', null, message),
      detail ? h('p', { class: 'small muted' }, detail) : null,
      action ? h('div', { class: 'toast-actions' }, actionEl(action, dismiss)) : null,
    ),
    iconButton({ iconName: 'close', label: 'この通知を閉じる', onClick: dismiss }),
  );
  if (kind !== 'error') {
    // 操作つきの知らせは、押すまでの時間を長く取る（4秒では、読んでからキーで届く前に消えた）。
    const life = ms ?? (action ? ACTION_MS : SUCCESS_MS);
    const arm = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (!hovered && !item.contains(document.activeElement)) dismiss();
        else arm();
      }, life);
    };
    item.addEventListener('mouseenter', () => {
      hovered = true;
    });
    item.addEventListener('mouseleave', () => {
      hovered = false;
    });
    arm();
  }
  region.append(item);
  if (key) for (const old of [...region.children]) if (old !== item && live.get(old) && live.get(old).key === key) drop(old);
  // 古い成功の通知から消して、画面を埋めないようにする。エラーは残す。
  const items = [...region.children];
  if (items.length > MAX_VISIBLE) {
    const old = items.find((x) => !x.classList.contains('toast--error'));
    if (old) drop(old);
  }
  place(true);
  choose = true;
  trackBars();
  return dismiss;
}

/**
 * 例: toast.success('No.98 を登録しました', { action: { label: '仕訳を見る', href: '#/journal/e_..' } })
 *     toast.success('元に戻せます', { action: { label: '元に戻す', onClick: undo }, ms: 10000 })
 *     toast.error(err.message)
 * ms は表示の長さ（ミリ秒）。既定は4秒、操作つきは10秒。エラーには効かない（閉じるまで残す）。
 * replaceKey は、同じ印の前の通知を置き換える（例: 続けて無視した行の「元に戻す」を、1つだけ出す）。
 * どれも、その通知を閉じる関数を返す（保存に成功したら、そのときの失敗の通知を閉じる、など）。
 */
export const toast = {
  success: (message, opts) => show('success', message, opts),
  info: (message, opts) => show('info', message, opts),
  error: (message, opts) => show('error', message, opts),
};
