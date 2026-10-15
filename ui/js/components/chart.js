// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 月次の推移のグラフ（棒: 売上と経費、線: 利益）。自作のSVGで、外部のライブラリは使わない。
// 経費の棒は斜線の模様で塗り、利益の点は赤字の月だけ塗りつぶす。色だけで意味を伝えないため。
// 収益も経費も0の月（記録のない月）は、点も線も描かず、線を切る（0へ急に落ちたように見えるため）。
// 読み上げには要約の文（aria-label）を付け、「表で見る」で同じ数字の表に切り替えられる
// （すぐ下に同じ数字の表を置く画面では、tableToggle: false で切替を出さない）。

import { h, s, uid } from '../dom.js';
import { formatYen, formatCompactYen } from '../format.js';
import { createTable } from './table.js';
import { emptyValue } from './money.js';
import { phrase } from './phrase.js';

const HEIGHT = 240;
const PAD = { top: 12, right: 12, bottom: 28, left: 52 };

/** 目盛りの間隔を、1・2・5 × 10^n の切りのよい値にする。 */
export function niceStep(range, count = 4) {
  if (range <= 0) return 1;
  const raw = range / count;
  const pow = 10 ** Math.floor(Math.log10(raw));
  const f = raw / pow;
  const nice = f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10;
  return nice * pow;
}

/** 記録のない月（収益も経費も0）。グラフでは点も線も描かない。 */
export function isEmptyMonth(r) {
  return !r.revenue && !r.expense;
}

/**
 * 利益の線を、記録のない月で切った、ひと続きの区間に分ける。until（YYYY-MM）より後の月は入れない。
 * @returns {Array<Array<{ index: number, month: string, profit: number }>>}
 */
export function profitSegments(series, until) {
  const out = [];
  let run = [];
  series.forEach((r, index) => {
    const skip = (until && r.month > until) || isEmptyMonth(r);
    if (skip) {
      if (run.length) out.push(run);
      run = [];
      return;
    }
    run.push({ index, month: r.month, profit: r.profit });
  });
  if (run.length) out.push(run);
  return out;
}

/**
 * 縦軸の範囲と目盛りの間隔。上は間隔の倍数まで、0より下は間隔の5分の1の刻みで、いちばん小さい値まで。
 * 小さな赤字のために、軸の下に大きな空きを作らないため（-2.5万のために -50万まで伸ばさない）。
 * @param {number[]} values 描く値（0 を含む）
 */
export function axisRange(values) {
  let lo = Math.min(0, ...values);
  let hi = Math.max(0, ...values);
  if (hi === lo) hi = lo + 10000;
  const step = niceStep(hi - lo);
  const minor = step / 5;
  lo = Math.floor(lo / minor) * minor;
  hi = Math.ceil(hi / step) * step;
  return { lo, hi, step };
}

/** 読み上げ用の要約。until（YYYY-MM）より後の月と、記録のない月は数えない。 */
export function summarize(series, until) {
  const rows = series.filter((r) => (!until || r.month <= until) && !isEmptyMonth(r));
  if (!rows.length) return '表示できる月がありません。';
  const sum = (k) => rows.reduce((a, r) => a + r[k], 0);
  const label = (r) => `${Number(r.month.slice(5, 7))}月`;
  let best = rows[0];
  let worst = rows[0];
  for (const r of rows) {
    if (r.profit > best.profit) best = r;
    if (r.profit < worst.profit) worst = r;
  }
  const year = rows[0].month.slice(0, 4);
  const range = `${year}年${label(rows[0])}〜${label(rows[rows.length - 1])}`;
  const worstText = `${label(worst)}の${formatYen(worst.profit)}円${worst.profit < 0 ? '（赤字）' : ''}`;
  return `${range}の推移。売上の合計 ${formatYen(sum('revenue'))}円、経費の合計 ${formatYen(sum('expense'))}円、利益の合計 ${formatYen(sum('profit'))}円。利益が最も多いのは${label(best)}の${formatYen(best.profit)}円、最も少ないのは${worstText}です。`;
}

// 凡例は名前だけにし、棒・線・点の見分け方は、凡例の下の1行にまとめる（括弧の説明が長く、凡例が何段にも割れていた）。
function legend(hatchId, highlight) {
  const swatch = (child) => s('svg', { width: 16, height: 12, viewBox: '0 0 16 12', 'aria-hidden': 'true' }, child);
  return h(
    'ul',
    { class: 'chart-legend' },
    h('li', null, swatch(s('rect', { x: 0, y: 0, width: 16, height: 12, rx: 2, class: 'chart-bar--revenue' })), '売上'),
    h('li', null, swatch(s('rect', { x: 0.5, y: 0.5, width: 15, height: 11, rx: 2, fill: `url(#${hatchId})`, class: 'chart-bar--expense' })), '経費'),
    h('li', null, swatch([s('path', { d: 'M1 6h14', class: 'chart-line' }), s('circle', { cx: 8, cy: 6, r: 3, class: 'chart-dot' })]), '利益'),
    // 見本は地の色に近いので、枠を付けて見分けられるようにする（ダークでも）。
    highlight ? h('li', null, swatch(s('rect', { x: 0.5, y: 0.5, width: 15, height: 11, rx: 2, class: 'chart-highlight chart-swatch-outline' })), '選んだ月') : null,
  );
}

function legendNote(highlight) {
  return `塗りの棒が売上、斜線の棒が経費、線が利益です。赤字の月は、点を塗って額を添えます。${highlight ? '網かけは選んだ月です。' : ''}`;
}

const LOSS_FONT = 11;

/** 額の文字のおよその幅（px）。数字と記号は文字の大きさの0.6倍、「万」「億」は1倍とみなす。 */
export function labelWidth(text, font = LOSS_FONT) {
  return [...text].reduce((w, ch) => w + (/[\x20-\x7e]/.test(ch) ? 0.6 : 1) * font, 0);
}

/** 額の段の高さ（文字の高さ＋すき間）。額は、グラフの枠の下の帯に、月の列の真下から並べる。 */
const LOSS_ROW = LOSS_FONT + 3;

/**
 * 赤字の月に添える額の文字を、置く段を決める（DOM不要。試験用に外へ出している）。
 * 額は、グラフの枠の下（月の名前の上）の帯に、その月の列の真下へ置く。棒や点の上に載せると、斜線の棒と重なって読めなかった。
 * 左の月から順に、横に重ならない最初の段（0、1…）へ置く。隣り合う月で幅が足りなければ、次の段へずらす。
 * 段は maxRows まで。それでも置けない月は省く（点の塗りは残る）。
 * @param {Array<{ index: number, profit: number }>} points 利益の点（profitSegments の中身）
 * @param {(i: number) => number} xOf 月の真ん中の x
 * @param {{ left: number, right: number }} bounds 文字を置ける左右の端
 * @param {number} [maxRows] 段の数の上限
 * @returns {Array<{ index: number, x: number, text: string, row: number }>} 左から順
 */
export function lossLabels(points, xOf, bounds, maxRows = 2) {
  const ends = [];
  const out = [];
  for (const p of points.filter((q) => q.profit < 0).sort((a, b) => a.index - b.index)) {
    const text = formatCompactYen(p.profit);
    const half = labelWidth(text) / 2 + 2;
    const x = Math.min(Math.max(xOf(p.index), bounds.left + half), bounds.right - half);
    let row = ends.findIndex((end) => x - half >= end);
    if (row < 0) {
      if (ends.length >= maxRows) continue;
      row = ends.length;
    }
    ends[row] = x + half;
    out.push({ index: p.index, x, text, row });
  }
  return out;
}

/**
 * グラフを作る。
 * 例: const c = createChart({ series, until: '2026-10', highlight: '2026-10', label: '2026年の推移' }); root.append(c.el)
 * @param {{ series: Array<{ month: string, revenue: number, expense: number, profit: number }>, until?: string,
 *           highlight?: string, label: string, tableToggle?: boolean }} opts
 *   tableToggle: 偽なら「表で見る」を出さない（すぐ下に同じ数字の表を置く画面のため）。既定は真。
 */
export function createChart(opts) {
  let { series, until, highlight } = opts;
  const withToggle = opts.tableToggle !== false;
  const hatchId = uid('hatch');
  // 模様の定義は凡例のSVGに置き、グラフの棒からも同じ id で参照する。
  const defsSvg = s(
    'svg',
    { width: 0, height: 0, 'aria-hidden': 'true', class: 'visually-hidden' },
    s(
      'defs',
      null,
      s(
        'pattern',
        { id: hatchId, width: 6, height: 6, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' },
        s('rect', { width: 6, height: 6, class: 'chart-hatch-bg' }),
        s('line', { x1: 1, y1: 0, x2: 1, y2: 6, class: 'chart-hatch-line' }),
      ),
    ),
  );
  const svgHost = h('div', { class: 'chart-plot' });
  const tableHost = h('div', { class: 'chart-table', hidden: true });
  // 名前を押した後の行動（「表で見る」「グラフで見る」）にする。押下の状態（aria-pressed）は付けない
  // （名前と一緒に変わると「グラフで見る、押されている」と読まれ、いまどちらの表示か分かりにくかった）。
  const toggle = withToggle ? h('button', { type: 'button', class: 'btn btn--sm' }, '表で見る') : null;
  const el = h(
    'div',
    { class: 'chart' },
    defsSvg,
    svgHost,
    withToggle ? tableHost : null,
    h('div', { class: 'chart-foot' }, legend(hatchId, highlight), toggle),
    h('p', { class: 'chart-note' }, phrase(legendNote(highlight))),
  );
  let width = 0;

  function draw() {
    const w = Math.max(280, Math.round(width || svgHost.clientWidth || 640));
    const pw = w - PAD.left - PAD.right;
    const ph = HEIGHT - PAD.top - PAD.bottom;
    const visible = series.filter((r) => !until || r.month <= until);
    const values = visible.flatMap((r) => [r.revenue, r.expense, r.profit]).concat(0);
    const { lo, hi, step } = axisRange(values);
    const y = (v) => PAD.top + ((hi - v) / (hi - lo)) * ph;
    const gw = pw / Math.max(series.length, 1);
    const bw = Math.max(3, Math.min(gw * 0.3, 18));
    const children = [];
    // 利益の線は、記録のない月で切る（区間ごとに M から描き直す）。
    const px = (i) => PAD.left + gw * i + gw / 2;
    const segments = profitSegments(series, until);
    // 赤字の月の額は、枠の下の帯に置く（棒や点の上に載せると、斜線の棒と重なって読めなかった）。段の数だけ、グラフの高さを足し、
    // 月の名前をその下へずらす。
    const losses = lossLabels(segments.flat(), px, { left: PAD.left, right: w - PAD.right });
    const H = HEIGHT + losses.reduce((m, l) => Math.max(m, l.row + 1), 0) * LOSS_ROW;

    // 目盛りの線は、0 を通る step の倍数だけ（0より下の浅い部分には、線も数字も置かない）。
    for (let t = Math.ceil(lo / step) * step; t <= hi + step / 2; t += step) {
      const yy = Math.round(y(t)) + 0.5;
      children.push(s('line', { x1: PAD.left, x2: w - PAD.right, y1: yy, y2: yy, class: t === 0 ? 'chart-zero' : 'chart-grid' }));
      children.push(s('text', { x: PAD.left - 8, y: yy + 4, 'text-anchor': 'end', class: 'chart-label' }, formatCompactYen(t)));
    }
    const short = gw < 34;
    series.forEach((r, i) => {
      const gx = PAD.left + gw * i;
      const cx = gx + gw / 2;
      const m = Number(r.month.slice(5, 7));
      if (r.month === highlight) children.unshift(s('rect', { x: gx + 1, y: PAD.top, width: gw - 2, height: ph, rx: 4, class: 'chart-highlight' }));
      children.push(s('text', { x: cx, y: H - 8, 'text-anchor': 'middle', class: 'chart-label' }, short ? String(m) : `${m}月`));
      if (until && r.month > until) return;
      const bar = (v, x, cls, extra = {}) => {
        const top = Math.min(y(v), y(0));
        const height = Math.max(Math.abs(y(v) - y(0)), v === 0 ? 0 : 1);
        return s('rect', { x, y: top, width: bw, height, rx: 2, class: cls, ...extra });
      };
      if (isEmptyMonth(r)) return;
      children.push(bar(r.revenue, cx - bw - 1, 'chart-bar--revenue'));
      children.push(bar(r.expense, cx + 1, 'chart-bar--expense', { fill: `url(#${hatchId})` }));
    });
    for (const seg of segments) {
      children.push(s('path', { d: seg.map((p, k) => `${k ? 'L' : 'M'}${px(p.index).toFixed(1)} ${y(p.profit).toFixed(1)}`).join(' '), class: 'chart-line' }));
      for (const p of seg) children.push(s('circle', { cx: px(p.index), cy: y(p.profit), r: 3.5, class: ['chart-dot', p.profit < 0 && 'chart-dot--loss'] }));
    }
    // 赤字の月には、その月の列の真下（枠の下の帯）に額を添える（塗りの違いだけでは、0の線のすぐ下の小さな赤字を見落とした。朱は使わない）。
    // 隣り合う月で幅が足りなければ、次の段へずらす。
    const plotBottom = PAD.top + ph;
    for (const l of losses) {
      children.push(s('text', { x: l.x.toFixed(1), y: plotBottom + 2 + LOSS_FONT + l.row * LOSS_ROW, 'text-anchor': 'middle', class: 'chart-loss-label' }, l.text));
    }
    const svg = s('svg', { class: 'chart-svg', viewBox: `0 0 ${w} ${H}`, width: w, height: H, role: 'img', 'aria-label': `${opts.label}。${summarize(series, until)}` }, children);
    svgHost.replaceChildren(svg);
  }

  function drawTable() {
    if (!withToggle) return;
    const rows = series.filter((r) => !until || r.month <= until);
    const t = createTable({
      caption: `${opts.label}（表）`,
      stack: false,
      columns: [
        { key: 'month', label: '月', render: (r) => `${Number(r.month.slice(5, 7))}月` },
        // 記録のない月は「—」（0円が並ぶと、急に落ち込んだように見えるため）。
        { key: 'revenue', label: '売上', num: true, render: (r) => (isEmptyMonth(r) ? emptyValue() : formatYen(r.revenue)) },
        { key: 'expense', label: '経費', num: true, render: (r) => (isEmptyMonth(r) ? emptyValue() : formatYen(r.expense)) },
        { key: 'profit', label: '利益', num: true, render: (r) => (isEmptyMonth(r) ? emptyValue() : r.profit < 0 ? `${formatYen(r.profit)}（赤字）` : formatYen(r.profit)) },
      ],
      rows,
    });
    tableHost.replaceChildren(t.el);
  }

  if (toggle) {
    toggle.addEventListener('click', () => {
      const showTable = tableHost.hidden;
      tableHost.hidden = !showTable;
      svgHost.hidden = showTable;
      toggle.textContent = showTable ? 'グラフで見る' : '表で見る';
    });
  }

  let frame = 0;
  const ro = typeof ResizeObserver === 'function'
    ? new ResizeObserver((entries) => {
        const next = Math.round(entries[0].contentRect.width);
        if (next && next !== width) {
          width = next;
          cancelAnimationFrame(frame);
          frame = requestAnimationFrame(draw);
        }
      })
    : null;
  if (ro) ro.observe(svgHost);
  draw();
  drawTable();

  return {
    el,
    update(next) {
      ({ series, until, highlight } = { series, until, highlight, ...next });
      draw();
      drawTable();
    },
    destroy() {
      if (ro) ro.disconnect();
      cancelAnimationFrame(frame);
    },
  };
}
