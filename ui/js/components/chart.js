// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 月次の推移のグラフ（棒: 売上と経費、線: 利益）。自作のSVGで、外部のライブラリは使わない。
// 経費の棒は斜線の模様で塗り、利益の点は赤字の月だけ塗りつぶす。色だけで意味を伝えないため。
// 読み上げには要約の文（aria-label）を付け、「表で見る」で同じ数字の表に切り替えられる。

import { h, s, uid } from '../dom.js';
import { formatYen, formatCompactYen } from '../format.js';
import { createTable } from './table.js';

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

/** 読み上げ用の要約。until（YYYY-MM）より後の月は数えない。 */
export function summarize(series, until) {
  const rows = series.filter((r) => !until || r.month <= until);
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

function legend(hatchId, highlight) {
  const swatch = (child) => s('svg', { width: 16, height: 12, viewBox: '0 0 16 12', 'aria-hidden': 'true' }, child);
  return h(
    'ul',
    { class: 'chart-legend' },
    h('li', null, swatch(s('rect', { x: 0, y: 0, width: 16, height: 12, rx: 2, class: 'chart-bar--revenue' })), '売上（塗りの棒）'),
    h('li', null, swatch(s('rect', { x: 0.5, y: 0.5, width: 15, height: 11, rx: 2, fill: `url(#${hatchId})`, class: 'chart-bar--expense' })), '経費（斜線の棒）'),
    h(
      'li',
      null,
      swatch([s('path', { d: 'M1 6h14', class: 'chart-line' }), s('circle', { cx: 8, cy: 6, r: 3, class: 'chart-dot' })]),
      '利益（線。赤字の月は塗りの点）',
    ),
    highlight ? h('li', null, swatch(s('rect', { x: 0, y: 0, width: 16, height: 12, rx: 2, class: 'chart-highlight' })), '選んだ月（網かけ）') : null,
  );
}

/**
 * グラフを作る。
 * 例: const c = createChart({ series, until: '2026-10', highlight: '2026-10', label: '2026年の推移' }); root.append(c.el)
 * @param {{ series: Array<{ month: string, revenue: number, expense: number, profit: number }>, until?: string,
 *           highlight?: string, label: string }} opts
 */
export function createChart(opts) {
  let { series, until, highlight } = opts;
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
  const toggle = h('button', { type: 'button', class: 'btn btn--sm', 'aria-pressed': 'false' }, '表で見る');
  const el = h('div', { class: 'chart' }, defsSvg, svgHost, tableHost, h('div', { class: 'chart-foot' }, legend(hatchId, highlight), toggle));
  let width = 0;

  function draw() {
    const w = Math.max(280, Math.round(width || svgHost.clientWidth || 640));
    const pw = w - PAD.left - PAD.right;
    const ph = HEIGHT - PAD.top - PAD.bottom;
    const visible = series.filter((r) => !until || r.month <= until);
    const values = visible.flatMap((r) => [r.revenue, r.expense, r.profit]).concat(0);
    let lo = Math.min(...values);
    let hi = Math.max(...values);
    if (hi === lo) hi = lo + 10000;
    const step = niceStep(hi - lo);
    lo = Math.floor(lo / step) * step;
    hi = Math.ceil(hi / step) * step;
    const y = (v) => PAD.top + ((hi - v) / (hi - lo)) * ph;
    const gw = pw / Math.max(series.length, 1);
    const bw = Math.max(3, Math.min(gw * 0.3, 18));
    const children = [];

    for (let t = lo; t <= hi + step / 2; t += step) {
      const yy = Math.round(y(t)) + 0.5;
      children.push(s('line', { x1: PAD.left, x2: w - PAD.right, y1: yy, y2: yy, class: t === 0 ? 'chart-zero' : 'chart-grid' }));
      children.push(s('text', { x: PAD.left - 8, y: yy + 4, 'text-anchor': 'end', class: 'chart-label' }, formatCompactYen(t)));
    }
    const short = gw < 34;
    const points = [];
    series.forEach((r, i) => {
      const gx = PAD.left + gw * i;
      const cx = gx + gw / 2;
      const m = Number(r.month.slice(5, 7));
      if (r.month === highlight) children.unshift(s('rect', { x: gx + 1, y: PAD.top, width: gw - 2, height: ph, rx: 4, class: 'chart-highlight' }));
      children.push(s('text', { x: cx, y: HEIGHT - 8, 'text-anchor': 'middle', class: 'chart-label' }, short ? String(m) : `${m}月`));
      if (until && r.month > until) return;
      const bar = (v, x, cls, extra = {}) => {
        const top = Math.min(y(v), y(0));
        const height = Math.max(Math.abs(y(v) - y(0)), v === 0 ? 0 : 1);
        return s('rect', { x, y: top, width: bw, height, rx: 2, class: cls, ...extra });
      };
      children.push(bar(r.revenue, cx - bw - 1, 'chart-bar--revenue'));
      children.push(bar(r.expense, cx + 1, 'chart-bar--expense', { fill: `url(#${hatchId})` }));
      points.push({ x: cx, y: y(r.profit), loss: r.profit < 0 });
    });
    if (points.length) {
      children.push(s('path', { d: points.map((p, i) => `${i ? 'L' : 'M'}${p.x.toFixed(1)} ${p.y.toFixed(1)}`).join(' '), class: 'chart-line' }));
      for (const p of points) children.push(s('circle', { cx: p.x, cy: p.y, r: 3.5, class: ['chart-dot', p.loss && 'chart-dot--loss'] }));
    }
    const svg = s('svg', { class: 'chart-svg', viewBox: `0 0 ${w} ${HEIGHT}`, width: w, height: HEIGHT, role: 'img', 'aria-label': `${opts.label}。${summarize(series, until)}` }, children);
    svgHost.replaceChildren(svg);
  }

  function drawTable() {
    const rows = series.filter((r) => !until || r.month <= until);
    const t = createTable({
      caption: `${opts.label}（表）`,
      stack: false,
      columns: [
        { key: 'month', label: '月', render: (r) => `${Number(r.month.slice(5, 7))}月` },
        { key: 'revenue', label: '売上', num: true, render: (r) => formatYen(r.revenue) },
        { key: 'expense', label: '経費', num: true, render: (r) => formatYen(r.expense) },
        { key: 'profit', label: '利益', num: true, render: (r) => (r.profit < 0 ? `${formatYen(r.profit)}（赤字）` : formatYen(r.profit)) },
      ],
      rows,
    });
    tableHost.replaceChildren(t.el);
  }

  toggle.addEventListener('click', () => {
    const showTable = tableHost.hidden;
    tableHost.hidden = !showTable;
    svgHost.hidden = showTable;
    toggle.textContent = showTable ? 'グラフで見る' : '表で見る';
    toggle.setAttribute('aria-pressed', showTable ? 'true' : 'false');
  });

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
