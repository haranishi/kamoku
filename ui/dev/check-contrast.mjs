// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// tokens.css の色の組のコントラスト比を計算し、足りなければ非0で終わる。AC-11.2。
//   node ui/dev/check-contrast.mjs
// 文字は4.5:1以上、入力欄の枠（border-strong）とフォーカスの輪（primary）は3:1以上。ライトとダークの両方。
// 計算はWCAG 2.1の相対輝度の式。OSに従うダークと手動のダークの値が食い違っていないかも確かめる。

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

/** 「#RGB」「#RRGGBB」を [r, g, b]（0〜255）に。 */
export function parseHex(hex) {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(String(hex).trim());
  if (!m) throw new Error(`色の形が読めません: ${hex}`);
  const s = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
  return [0, 2, 4].map((i) => parseInt(s.slice(i, i + 2), 16));
}

function luminance([r, g, b]) {
  const lin = (v) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** 2色のコントラスト比（1〜21）。 */
export function contrast(a, b) {
  const la = luminance(parseHex(a));
  const lb = luminance(parseHex(b));
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

function block(css, selectorRe) {
  const m = selectorRe.exec(css);
  if (!m) return null;
  const start = css.indexOf('{', m.index) + 1;
  const end = css.indexOf('}', start);
  const vars = {};
  for (const d of css.slice(start, end).matchAll(/(--c-[a-z0-9-]+)\s*:\s*(#[0-9a-fA-F]{3,6})\s*;/g)) vars[d[1]] = d[2];
  return vars;
}

/** tokens.css から、ライト・OSのダーク・手動のダークの色を取り出す。 */
export function parseTokens(css) {
  return {
    light: block(css, /^:root\s*\{/m),
    darkOs: block(css, /:root:not\(\[data-theme="light"\]\)\s*\{/),
    darkManual: block(css, /:root\[data-theme="dark"\]\s*\{/),
  };
}

const TEXT_FG = ['--c-text', '--c-muted', '--c-primary', '--c-income', '--c-warn', '--c-danger'];
const TEXT_BG = ['--c-surface', '--c-bg', '--c-surface-2'];
const PAIRS_45 = [
  ...TEXT_FG.flatMap((fg) => TEXT_BG.map((bg) => [fg, bg])),
  ['--c-text', '--c-primary-soft'],
  ['--c-primary', '--c-primary-soft'],
  ['--c-on-primary', '--c-primary'],
  ['--c-on-danger', '--c-danger'],
  // 状態の文字を、状態の背景の上に。
  ['--c-text', '--c-pending-bg'],
  ['--c-warn', '--c-pending-bg'],
  ['--c-text', '--c-ok-bg'],
  ['--c-income', '--c-ok-bg'],
  ['--c-text', '--c-danger-bg'],
  ['--c-danger', '--c-danger-bg'],
];
// 部品ごとの文字と地（どこで使う組か）。上の総当たりと重なる組は、数えるときに1つにまとめる。
const PAIRS_45_USES = [
  ['--c-primary', '--c-primary-soft'], // 藍の札（badge--info）、サイドバーの件数
  ['--c-danger', '--c-danger-bg'], // 朱の札（訂正・取消。badge--danger）
  ['--c-muted', '--c-surface-2'], // 無彩色の札（赤字。badge--muted）、ショートカットの小枠（kbd）の文字
  ['--c-muted', '--c-surface'], // 試算表の薄い0（.rt-zero）。合計の行の地（surface-2）は上の組
  ['--c-muted', '--c-primary-soft'], // 選んだ行・未確認の確定候補の行・藍の帯の上の、補足の文字
  ['--c-muted', '--c-ok-bg'], // 一致の札の地の上の、補足の文字
];
const PAIRS_3 = [
  ['--c-border-strong', '--c-surface'],
  ['--c-border-strong', '--c-bg'],
  ['--c-primary', '--c-surface'],
  ['--c-primary', '--c-bg'],
  // チェックボックスと入力欄の枠が、薄い地の行（合計の行、指を置いた行）と、未確認の確定候補の行（藍の淡い地）に載る。
  ['--c-border-strong', '--c-surface-2'],
  ['--c-border-strong', '--c-primary-soft'],
];

/**
 * すべての組を計算する。
 * @returns {{ rows: Array<{ theme, fg, bg, ratio, need, ok }>, problems: string[] }}
 */
export function checkTokens(css) {
  const t = parseTokens(css);
  const problems = [];
  if (!t.light || !t.darkOs || !t.darkManual) problems.push('tokens.css に、ライト・OSのダーク・手動のダークの3つの定義が揃っていません。');
  if (t.darkOs && t.darkManual) {
    for (const k of new Set([...Object.keys(t.darkOs), ...Object.keys(t.darkManual)])) {
      if (t.darkOs[k] !== t.darkManual[k]) problems.push(`ダークの ${k} が、OSに従う定義（${t.darkOs[k]}）と手動の定義（${t.darkManual[k]}）で違います。`);
    }
  }
  const rows = [];
  for (const [theme, vars] of [['ライト', t.light], ['ダーク', t.darkManual]]) {
    if (!vars) continue;
    const seen = new Set();
    for (const [pairs, need] of [[PAIRS_45, 4.5], [PAIRS_45_USES, 4.5], [PAIRS_3, 3]]) {
      for (const [fg, bg] of pairs) {
        if (seen.has(`${fg}|${bg}|${need}`)) continue;
        seen.add(`${fg}|${bg}|${need}`);
        if (!vars[fg] || !vars[bg]) {
          problems.push(`${theme}: ${fg} か ${bg} が定義されていません。`);
          continue;
        }
        const ratio = contrast(vars[fg], vars[bg]);
        const ok = ratio >= need;
        rows.push({ theme, fg, bg, ratio, need, ok });
        if (!ok) problems.push(`${theme}: ${fg} / ${bg} は ${ratio.toFixed(2)}:1 で、${need}:1 に足りません。`);
      }
    }
  }
  return { rows, problems };
}

function main() {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const css = readFileSync(path.join(here, '..', 'css', 'tokens.css'), 'utf8');
  const { rows, problems } = checkTokens(css);
  for (const r of rows) {
    console.log(`${r.ok ? 'OK ' : 'NG '} ${r.theme} ${r.fg.padEnd(18)} / ${r.bg.padEnd(16)} ${r.ratio.toFixed(2).padStart(5)}:1（${r.need}:1 以上）`);
  }
  if (problems.length) {
    console.error(`\nコントラストの問題が ${problems.length} 件あります。`);
    for (const p of problems) console.error(`- ${p}`);
    process.exit(1);
  }
  console.log(`\n${rows.length} 組すべて基準を満たしました（AC-11.2）。`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
