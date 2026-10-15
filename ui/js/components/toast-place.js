// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 通知（toast.js）の置き場所。候補の計算（toastCandidates）はDOMに触らない（試験のため）。
// 下に出すと、保存の帯のすぐ上の欄（記帳でこれから入る欄）や、未確認のこれから進む行に重なった。
// 上に出せば、重なるのは、たいてい済んだ所（上の行、残した日付）になる。そこで上を先に試し、フォーカスのある欄に
// 重なるときだけ、次の候補（下）へ移す（createPlacement が、実際に置いて重なりを確かめる）。
// 広い幅の上（本文の列の上の真ん中）は、そこに押せる物（見出しの横のボタン、月の選び方など）があれば使わず、今までの右下にする
// （avoidControls。上の帯が無いので、768px の幅では見出しの横のボタンに重なりやすい）。
// 小窓が開いているときは、小窓の外に余白があれば外へ（背景は押せないので、何も隠さない）、無ければ見出しの下か、下の端へ。
// 見出し（閉じる）と下のボタンの並び（保存・確定）には、どの候補も重ならない。

const GAP = 8;
const EDGE = 16;
const WIDE_EDGE = 24;
const MAX_W = 440;
const MIN_W = 280;

/**
 * 候補を、試す順に並べる。数は px。null は「指定しない（auto）」。'bottom' は、今までの下の位置（CSS の既定）。
 * @param {{ vw: number, vh: number, narrow: boolean, topbarBottom: number, column?: { left: number, right: number } | null,
 *           dialog?: { left: number, right: number, top: number, bottom: number, headBottom: number, footTop?: number | null } | null }} g
 *   topbarBottom: 上の帯の下の端（帯が無ければ 0）。column: 本文の列の左右の端。dialog: いちばん手前の小窓の四角と、見出しの下の端・下のボタンの並びの上の端。
 * @returns {Array<{ key: string, top?: number|null, bottom?: number|null, left?: number|null, right?: number|null, maxWidth?: number|null, avoidControls?: boolean }>}
 *   avoidControls: 真なら、フォーカスのある要素だけでなく、押せる物のどれかに重なる位置も使わない。
 */
export function toastCandidates(g) {
  const d = g.dialog;
  if (!d) {
    const top = g.narrow
      ? { key: 'top', top: (g.topbarBottom > 0 ? g.topbarBottom + GAP : EDGE), left: EDGE, right: EDGE, maxWidth: null }
      : { key: 'top', top: EDGE, left: (g.column ? g.column.left : 0) + WIDE_EDGE, right: g.vw - (g.column ? g.column.right : g.vw) + WIDE_EDGE, maxWidth: MAX_W, avoidControls: true };
    return [top, { key: 'bottom' }];
  }
  const out = [];
  const roomRight = g.vw - d.right;
  const roomLeft = d.left;
  if (roomRight >= MIN_W + 2 * WIDE_EDGE) out.push({ key: 'out-right', top: EDGE, right: WIDE_EDGE, left: null, maxWidth: Math.min(MAX_W, roomRight - 2 * WIDE_EDGE) });
  if (roomLeft >= MIN_W + 2 * WIDE_EDGE) out.push({ key: 'out-left', top: EDGE, right: g.vw - d.left + WIDE_EDGE, left: null, maxWidth: Math.min(MAX_W, roomLeft - 2 * WIDE_EDGE) });
  const inside = { left: d.left + EDGE, right: g.vw - d.right + EDGE, maxWidth: Math.min(MAX_W, Math.max(d.right - d.left - 2 * EDGE, 0)) };
  out.push({ key: 'in-top', top: d.headBottom + GAP, ...inside });
  out.push({ key: 'in-bottom', bottom: g.vh - (d.footTop ?? d.bottom) + GAP, ...inside });
  return out;
}

/** 2つの四角が重なるか（辺が触れるだけは重ならない）。 */
export function overlaps(a, b) {
  return Boolean(a && b) && a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom;
}

const rectOf = (el) => (el && typeof el.getBoundingClientRect === 'function' ? el.getBoundingClientRect() : null);

/** 置き場所を決めるための、画面・上の帯・本文の列・手前の小窓（host が home でなければ）の寸法。 */
function geometry(host, home) {
  const bar = rectOf(document.querySelector('.topbar'));
  const col = rectOf(document.querySelector('.main-col'));
  let dialog = null;
  if (host && host !== home) {
    const r = rectOf(host);
    const head = rectOf(host.querySelector('.dialog-head'));
    const foot = rectOf(host.querySelector('.dialog-foot'));
    if (r) dialog = { left: r.left, right: r.right, top: r.top, bottom: r.bottom, headBottom: head ? head.bottom : r.top, footTop: foot ? foot.top : null };
  }
  return {
    vw: window.innerWidth,
    vh: window.innerHeight,
    narrow: window.matchMedia('(max-width: 767px)').matches,
    topbarBottom: bar && bar.height > 0 && bar.bottom > 0 ? bar.bottom : 0,
    column: col && col.width > 0 ? { left: col.left, right: col.right } : null,
    dialog,
  };
}

/** フォーカスのある要素の四角（チェックとラジオは包むラベル）。通知の中や、どこにも無いときは null。 */
function focusRect(region) {
  const el = document.activeElement;
  if (!el || el === document.body || el === document.documentElement || region.contains(el)) return null;
  const box = (typeof el.matches === 'function' && el.matches('input[type="checkbox"], input[type="radio"]') && el.closest('label')) || el;
  const r = rectOf(box);
  return r && (r.width || r.height) ? r : null;
}

const CONTROLS = 'main a[href], main button, main input:not([type="hidden"]), main select, main textarea, main [role="tab"], main [tabindex="0"]';

/** 本文の押せる物のどれかが、四角 r に重なるか。 */
function controlsUnder(r) {
  return [...document.querySelectorAll(CONTROLS)].some((el) => {
    const b = el.getBoundingClientRect();
    return b.width > 0 && b.height > 0 && overlaps(r, b);
  });
}

/** 候補の位置を入れ物に当てる（変数と data-place）。'bottom' は、印を外して、CSS の既定（下）に戻す。 */
function applyPlace(region, c) {
  if (c.key === 'bottom') {
    if (region.dataset.place) delete region.dataset.place;
    return;
  }
  const set = (name, value) => {
    if (region.style.getPropertyValue(name) !== value) region.style.setProperty(name, value);
  };
  const px = (v) => (v === null || v === undefined ? 'auto' : `${Math.round(v)}px`);
  set('--toast-top', px(c.top));
  set('--toast-bottom', px(c.bottom));
  set('--toast-left', px(c.left));
  set('--toast-right', px(c.right));
  set('--toast-maxw', c.maxWidth ? `${Math.round(c.maxWidth)}px` : 'none');
  if (region.dataset.place !== c.key) region.dataset.place = c.key;
}

/**
 * 置き場所を受け持つ。選んだ候補（key）と、そのときの置き先を覚える。候補は、出したときとフォーカスが動いたときに選び直し、
 * スクロールなどでは同じ候補のまま位置だけ直す（読んでいる途中で、通知が上下に跳ねないように）。
 */
export function createPlacement() {
  let placeKey = null;
  let placedIn = null;
  return {
    /**
     * 候補を順に当て、フォーカスのある要素に重ならない（avoidControls の候補は、押せる物にも重ならない）最初の候補にする。
     * 無ければ、フォーカスに重ならない最初の候補。それも無ければ最初の候補。
     * again が偽なら、前に選んだ候補のまま位置だけ直す（小窓が開いたり閉じたりしたら、選び直す）。
     */
    place(region, host, home, again) {
      if (typeof window.innerWidth !== 'number' || !rectOf(region)) return;
      const list = toastCandidates(geometry(host, home));
      // 通知のボタン（「元に戻す」など）にフォーカスがある間は、選び直さない（押そうとしている通知が跳ねないように）。
      const inside = region.contains(document.activeElement);
      const kept = (!again || inside) && host === placedIn && list.find((c) => c.key === placeKey);
      placedIn = host;
      if (kept) {
        applyPlace(region, kept);
        return;
      }
      const focus = focusRect(region);
      const fits = (strict) =>
        list.find((c) => {
          applyPlace(region, c);
          const r = rectOf(region);
          return !overlaps(r, focus) && !(strict && c.avoidControls && controlsUnder(r));
        });
      const pick = fits(true) || fits(false) || list[0];
      applyPlace(region, pick);
      placeKey = pick.key;
    },
    /** 通知が無くなったら、印を外して、次に出すときに選び直す。 */
    reset(region) {
      if (region.dataset && region.dataset.place) delete region.dataset.place;
      placeKey = null;
      placedIn = null;
    },
  };
}
