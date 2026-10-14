// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 読込中の骨組み。300ms以上かかるときだけ出す（screens-other.md）。すぐ終わる読み込みで画面がちらつかないように。

import { h } from '../dom.js';

/** 骨組みの出し始めまでの待ち時間（ミリ秒）。 */
export const SKELETON_DELAY_MS = 300;

/**
 * 灰色の帯を並べた骨組み。
 * @param {{ lines?: number, widths?: string[], label?: string }} [opts]
 */
export function skeletonBlock({ lines = 3, widths = ['40%', '90%', '70%', '85%', '60%'], label = '読み込んでいます' } = {}) {
  const block = h('div', { class: 'skel-block', role: 'status', 'aria-label': label });
  for (let i = 0; i < lines; i += 1) {
    const bar = h('span', { class: 'skel' });
    bar.style.width = widths[i % widths.length];
    block.append(bar);
  }
  return block;
}

/**
 * 読み込みが300msを超えたら、container に骨組みを入れる。止める関数を返す。
 * 例: const stop = delayedSkeleton(main, () => skeletonBlock({ lines: 4 })); await load(); stop();
 * @param {HTMLElement} container
 * @param {() => Node} make
 * @param {number} [delay]
 */
export function delayedSkeleton(container, make, delay = SKELETON_DELAY_MS) {
  let shown = null;
  const timer = setTimeout(() => {
    shown = make();
    container.append(shown);
  }, delay);
  return () => {
    clearTimeout(timer);
    if (shown) shown.remove();
  };
}
