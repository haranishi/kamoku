// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面のソースが「守る決まり」に反していないかを、文字列の検索で確かめる。違反があれば非0で終わる。
//   node ui/dev/check-policy.mjs
// 決まり（u1 の依頼書）: CSPで動く（インラインのスクリプト・スタイル、style属性を使わない）、
// innerHTML などを使わない、外のURLを書かない、通信は Rust の API 境界だけ、色は tokens.css の変数だけ、
// 絵文字をアイコンにしない、IMEの判定は Rust の共通キー処理だけ（keyboard.md）。

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

// SVGの名前空間はXMLの識別子で、取りに行くURLではない。これだけを例外として許す。
const ALLOWED_URLS = new Set(['http://www.w3.org/2000/svg']);

const JS_RULES = [
  [/\binnerHTML\b/, 'innerHTML を使っています。dom.js の h() で要素を作ってください。'],
  [/\bouterHTML\b/, 'outerHTML を使っています。'],
  [/\binsertAdjacentHTML\b/, 'insertAdjacentHTML を使っています。'],
  [/\bdocument\.write(ln)?\s*\(/, 'document.write を使っています。'],
  [/(^|[^.\w])eval\s*\(/, 'eval を使っています。'],
  [/\bnew\s+Function\s*\(/, 'new Function を使っています。'],
  [/setAttribute\(\s*['"]style['"]/, "setAttribute('style', …) はCSPで止まります。el.style.setProperty を使ってください。"],
  [/\.style\.cssText\b/, 'style.cssText は使わず、el.style.setProperty を使ってください。'],
  [/\bsrcdoc\b/, 'srcdoc を使っています。'],
  [/\b(XMLHttpRequest|WebSocket|EventSource)\b|sendBeacon/, '通信は api.js の fetch だけにしてください。'],
];

const HTML_RULES = [
  [/<script(?![^>]*\bsrc\s*=)[^>]*>/i, 'インラインの <script> があります。ファイルに分けてください。'],
  [/<style\b/i, 'インラインの <style> があります。css/ に分けてください。'],
  [/\sstyle\s*=/i, 'style 属性があります。CSSのクラスにしてください。'],
  [/\son[a-z]+\s*=/i, 'インラインのイベント（onclick など）があります。CSPで止まります。'],
  [/javascript:/i, 'javascript: のURLがあります。'],
];

/**
 * 1つのファイルを調べる。違反の一覧を返す。
 * @param {string} rel ui/ からの相対パス（例: 'js/api.js'）
 * @param {string} text 中身
 * @returns {Array<{ file: string, line: number, message: string }>}
 */
export function checkSource(rel, text) {
  const out = [];
  const ext = path.extname(rel);
  const production = path.extname(rel) === '.rs' ? text.split('#[cfg(test)]')[0] : text;
  const lines = production.split('\n');
  const isJs = ext === '.js' || ext === '.mjs';
  const isRust = ext === '.rs';
  const isCss = ext === '.css';
  const isHtml = ext === '.html';
  lines.forEach((raw, i) => {
    const line = i + 1;
    const add = (message) => out.push({ file: rel, line, message });
    // Native regression cases deliberately contain hostile URLs and strings.
    for (const m of raw.matchAll(/https?:\/\/[^\s'"`)<>]*/g)) {
      if (!ALLOWED_URLS.has(m[0])) add(`外のURL（${m[0]}）を書いています。画面は外へ通信しません。`);
    }
    // コメントの行（// /* * で始まる行）は、使い方の説明に関数名が出てくるので、URL以外の規則の対象から外す。
    const trimmed = raw.trim();
    if ((isJs || isCss || isRust) && (trimmed.startsWith('//') || trimmed.startsWith('/*') || trimmed.startsWith('*'))) return;
    if (isJs || isRust) {
      for (const [re, message] of JS_RULES) if (re.test(raw)) add(message);
      if (/\bfetch\s*\(/.test(raw)) add('通信はRustのAPI境界からだけ行ってください。');
      if (isRust && /fetch_with_|Request::new|RequestInit::new/.test(raw) && !/\/(app\/core|browser)\/api\.rs$/.test(rel)) add('通信はRustのAPI境界からだけ行ってください。');
      if (isRust && /set_inner_html|set_outer_html|insert_adjacent_html|js_sys::eval|Function::new/.test(raw)) add('文字列をHTMLやJavaScriptとして実行しないでください。');
      if (isRust && /\.(is_composing|key_code)\(/.test(raw) && !/\/(app\/core\/keys|browser\/events|interaction)\.rs$/.test(rel)) add('IMEの判定は共通キー処理を使ってください。');
      if (/['"`]#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3})?(?:[0-9a-fA-F]{2})?['"`]/.test(raw)) add('16進の色を書いています。tokens.css の変数を使ってください。');
    }
    if (isHtml) for (const [re, message] of HTML_RULES) if (re.test(raw)) add(message);
    if (isCss && rel !== 'css/tokens.css') {
      const code = raw.replace(/\/\*.*?\*\//g, '');
      if (/#[0-9a-fA-F]{3,8}\b/.test(code)) add('部品のCSSに16進の色があります。tokens.css の変数を使ってください。');
      if (/\b(rgba?|hsla?)\s*\(/.test(code)) add('部品のCSSに色の関数（rgb・hsl）があります。tokens.css の変数を使ってください。');
    }
    if ((isJs || isHtml) && /\p{Extended_Pictographic}/u.test(raw)) add('絵文字があります。アイコンは js/icons.js の自作のSVGを使ってください。');
  });
  return out;
}

function walk(dir, base, acc) {
  for (const name of readdirSync(dir)) {
    const full = path.join(dir, name);
    const rel = path.relative(base, full).split(path.sep).join('/');
    if (statSync(full).isDirectory()) walk(full, base, acc);
    else if (/\.(js|mjs|css|html|svg|rs)$/.test(name)) acc.push(rel);
  }
  return acc;
}

/** 画面のソース（index.html、favicon.svg、css/、js/）の一覧。dev/ と tests/ は Node の道具なので対象外。 */
export function uiSources(uiRoot) {
  // Generated wasm-bindgen bindings have their own toolchain provenance. Scan
  // authored preview assets, while keeping generated third-party code separate.
  const files = ['index.html', 'favicon.svg', 'boot.js', 'rust/index.html', 'rust/boot.js', 'rust/preview.css'];
  walk(path.join(uiRoot, 'css'), uiRoot, files);
  walk(path.resolve(uiRoot, '../crates/kamoku-ui/src'), uiRoot, files);
  return files;
}

function main() {
  const uiRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const files = uiSources(uiRoot);
  const problems = files.flatMap((rel) => checkSource(rel, readFileSync(path.join(uiRoot, rel), 'utf8')));
  if (problems.length) {
    for (const p of problems) console.error(`ui/${p.file}:${p.line}: ${p.message}`);
    console.error(`\n決まりへの違反が ${problems.length} 件あります。`);
    process.exit(1);
  }
  console.log(`${files.length} ファイルを調べ、決まりへの違反はありませんでした。`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
