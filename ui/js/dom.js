// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面の要素は、すべてここの h() と s() で作る。
// innerHTML を使わないのは、取引先や摘要の文字列がHTMLとして解釈される道を、最初から無くすため。

/** SVGの名前空間。XMLの識別子で、ブラウザが取りに行くURLではない（check-policy で許可している唯一の例外）。 */
export const SVG_NS = 'http://www.w3.org/2000/svg';

// 属性ではなくプロパティで入れるもの。value を属性で入れると、利用者が打った後に表示が戻らない。
const PROPS = new Set(['value', 'checked', 'indeterminate']);

function applyProps(el, props, isSvg) {
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null || value === false) continue;
    if (key === 'style') {
      // CSPで style 属性を禁じているので、渡されたら作る側の誤り。黙って捨てずに知らせる。
      throw new Error('h(): style は使えません。class か el.style.setProperty を使ってください');
    }
    if (key === 'class' || key === 'className') {
      const names = Array.isArray(value) ? value.flat(Infinity).filter(Boolean).join(' ') : String(value);
      if (names) el.setAttribute('class', names);
    } else if (key === 'text') {
      el.textContent = String(value);
    } else if (key === 'dataset') {
      for (const [k, v] of Object.entries(value)) {
        if (v !== undefined && v !== null) el.dataset[k] = String(v);
      }
    } else if (key.startsWith('on') && typeof value === 'function') {
      el.addEventListener(key.slice(2).toLowerCase(), value);
    } else if (!isSvg && PROPS.has(key)) {
      el[key] = value;
    } else if (value === true) {
      el.setAttribute(key, '');
    } else {
      el.setAttribute(key, String(value));
    }
  }
}

function appendChildren(el, children) {
  for (const child of children.flat(Infinity)) {
    if (child === undefined || child === null || child === false || child === true) continue;
    el.append(typeof child === 'object' ? child : String(child));
  }
}

/**
 * HTMLの要素を作る。
 * 例: h('button', { class: 'btn', type: 'button', onclick: save }, '保存')
 * @param {string} tag
 * @param {Record<string, any>|null} [props]
 * @param {...any} children 文字列・要素・配列（null と false は飛ばす）
 * @returns {HTMLElement}
 */
export function h(tag, props, ...children) {
  const el = document.createElement(tag);
  if (props) applyProps(el, props, false);
  appendChildren(el, children);
  return el;
}

/**
 * SVGの要素を作る。属性はすべて setAttribute で入れる。
 * @param {string} tag
 * @param {Record<string, any>|null} [attrs]
 * @param {...any} children
 * @returns {SVGElement}
 */
export function s(tag, attrs, ...children) {
  const el = document.createElementNS(SVG_NS, tag);
  if (attrs) applyProps(el, attrs, true);
  appendChildren(el, children);
  return el;
}

/** 子をすべて外す。 */
export function clear(el) {
  el.replaceChildren();
  return el;
}

/** 子を入れ替える。 */
export function replace(el, ...children) {
  el.replaceChildren();
  appendChildren(el, children);
  return el;
}

/** 片付けの関数を返す addEventListener。 */
export function listen(target, type, fn, options) {
  target.addEventListener(type, fn, options);
  return () => target.removeEventListener(type, fn, options);
}

let counter = 0;
/** 画面の中で重ならない id を作る（ARIAの結び付けに使う）。 */
export function uid(prefix = 'k') {
  counter += 1;
  return `${prefix}-${counter}`;
}

/**
 * 欄に入ったら、文字を全部選ぶ（上から打ち直せるように）。
 * クリックで入ったときは、mouseup がカーソルを置き直して選択を外すので、最初の mouseup だけ止める。
 */
export function selectAllOnFocus(input) {
  let pending = false;
  input.addEventListener('focus', () => {
    input.select();
    pending = true;
  });
  input.addEventListener('mouseup', (e) => {
    if (!pending) return;
    pending = false;
    e.preventDefault();
    input.select();
  });
  input.addEventListener('blur', () => {
    pending = false;
  });
}

/** 読み上げだけに出す文字。 */
export function srOnly(text) {
  return h('span', { class: 'visually-hidden' }, text);
}

/** aria-describedby に id を足す（重複は足さない）。 */
export function addDescribedBy(el, id) {
  const ids = new Set((el.getAttribute('aria-describedby') || '').split(/\s+/).filter(Boolean));
  ids.add(id);
  el.setAttribute('aria-describedby', [...ids].join(' '));
}

/** aria-describedby から id を外す。 */
export function removeDescribedBy(el, id) {
  const ids = (el.getAttribute('aria-describedby') || '').split(/\s+/).filter((x) => x && x !== id);
  if (ids.length) el.setAttribute('aria-describedby', ids.join(' '));
  else el.removeAttribute('aria-describedby');
}
