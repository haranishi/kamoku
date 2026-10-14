// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 見た目の設定（ライト/ダーク/OS、行の密度）。ブラウザの中にだけ保存する（screens-other.md の SC-10）。
// 保存できない環境（プライベートウィンドウなど）でも、画面は既定の見た目で動く。

const THEME_KEY = 'kamoku.theme';
const DENSITY_KEY = 'kamoku.density';
const THEMES = ['os', 'light', 'dark'];
const listeners = new Set();

function read(key) {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key, value) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // 保存できなくても、この画面の間は効く。
  }
}

/** いまの明暗の設定（'os' | 'light' | 'dark'）。 */
export function getTheme() {
  const v = read(THEME_KEY);
  return THEMES.includes(v) ? v : 'os';
}

/** いまの行の密度（'standard' | 'compact'）。 */
export function getDensity() {
  return read(DENSITY_KEY) === 'compact' ? 'compact' : 'standard';
}

/** 実際に見えている明暗（OSに従うときは、OSの設定を引く）。 */
export function effectiveTheme() {
  const t = getTheme();
  if (t !== 'os') return t;
  return typeof matchMedia === 'function' && matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/** 設定を <html> に当てる。 */
export function applyAppearance(root = document.documentElement) {
  const t = getTheme();
  if (t === 'os') root.removeAttribute('data-theme');
  else root.setAttribute('data-theme', t);
  if (getDensity() === 'compact') root.setAttribute('data-density', 'compact');
  else root.removeAttribute('data-density');
}

/** 明暗を変えて保存する。 */
export function setTheme(theme) {
  write(THEME_KEY, THEMES.includes(theme) ? theme : 'os');
  applyAppearance();
  listeners.forEach((fn) => fn());
}

/** OS → ライト → ダーク → OS の順に切り替える。 */
export function cycleTheme() {
  const next = THEMES[(THEMES.indexOf(getTheme()) + 1) % THEMES.length];
  setTheme(next);
  return next;
}

/** 行の密度を変えて保存する。 */
export function setDensity(density) {
  write(DENSITY_KEY, density === 'compact' ? 'compact' : 'standard');
  applyAppearance();
  listeners.forEach((fn) => fn());
}

/** 設定の名前（画面に出す文字）。 */
export function themeLabel(theme = getTheme()) {
  return { os: 'OSに合わせる', light: 'ライト', dark: 'ダーク' }[theme];
}

/** 設定が変わったときに呼ぶ関数を登録する。片付けの関数を返す。 */
export function onAppearanceChange(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}
