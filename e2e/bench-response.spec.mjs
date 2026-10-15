// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 速さの目標（nfr.md）「画面の操作の応答は100ミリ秒以内。未確認200行の一覧で、科目を打って候補を絞る」の測定。
// 打った文字の入力イベントから、候補が出て、次の描画の枠に入るまでを、ページの中の時計で測る
// （Playwright の往復は含めない）。結果は docs/benchmarks.md に載せる。合成データだけを使う。

import { test, expect } from '@playwright/test';
import { open, uniqueText, walletOf, importRows } from './helpers.mjs';

const ROWS = 200;
const WARMUPS = 3;
const SAMPLES = 20;

const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const p95 = (xs) => [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(xs.length * 0.95))];

test(`NFR 未確認${ROWS}行の一覧で、科目の欄に打ってから候補が出るまでの時間を測る`, async ({ page }, info) => {
  test.setTimeout(120_000);
  await open(page, '#/');
  const token = uniqueText('E2E 応答').replace(/\s+/g, '-');
  const card = await walletOf(page, 'card');
  const rows = Array.from({ length: ROWS }, (_, i) => [`2026-02-${String((i % 28) + 1).padStart(2, '0')}`, `${token} ${String(i + 1).padStart(3, '0')}`, String(300 + i * 10), '']);
  await importRows(page, card.code, rows);
  await open(page, `#/inbox?q=${encodeURIComponent(token)}`);
  await expect(page.locator('tr.inbox-row')).toHaveCount(ROWS);

  // 検索の欄から、最初の行の科目の欄へ（キーボードだけの流れ。inbox-bench.spec.mjs と同じ）。
  await page.keyboard.press('/');
  // 検索欄 → 見出しの「すべて選ぶ」→ 候補の欄 → 日付の欄 → 科目の欄（inbox-bench.spec.mjs と同じ）。
  for (let i = 0; i < 4; i += 1) await page.keyboard.press('Tab');

  // 打つ前に、入力イベントの時刻と、候補が出て次の描画の枠に入った時刻を、ページの中で記録する。
  const arm = () =>
    page.evaluate(() => {
      window.__response = null;
      let start = 0;
      const onInput = () => {
        start = performance.now();
        document.removeEventListener('input', onInput, true);
      };
      document.addEventListener('input', onInput, true);
      // 候補の入れ物は、行ごとに隠れた状態で最初から作られている。いま打っている欄の、開いた候補の先頭が選ばれたときを「出た」とする。
      const observer = new MutationObserver(() => {
        if (!start || window.__response !== null) return;
        const field = document.activeElement;
        const list = field && document.getElementById(field.getAttribute('aria-controls') || '');
        if (list && !list.hidden && list.querySelector('[role="option"][aria-selected="true"]')) {
          observer.disconnect();
          // 描画の枠が作られたあと（setTimeout）の時刻まで測る。次の枠の頭だけを測ると、実際より短く出る。
          requestAnimationFrame(() => {
            setTimeout(() => {
              window.__response = performance.now() - start;
            }, 0);
          });
        }
      });
      observer.observe(document.body, { childList: true, subtree: true, attributes: true });
    });

  const times = [];
  for (let n = 0; n < WARMUPS + SAMPLES; n += 1) {
    await arm();
    await page.keyboard.insertText('つうし');
    await page.waitForFunction(() => window.__response !== null);
    const ms = await page.evaluate(() => window.__response);
    if (n >= WARMUPS) times.push(ms);
    // 次の試行のために、欄を空にして、候補を閉じる。
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.press('Backspace');
    await page.keyboard.press('Escape');
    await expect(page.locator(':focus')).toHaveAttribute('aria-expanded', 'false');
  }

  const text = `未確認${ROWS}行、科目の読みを3文字打ってから候補が出て次の描画の枠に入るまで（${SAMPLES}回）: 中央値 ${median(times).toFixed(1)}ミリ秒、95% ${p95(times).toFixed(1)}ミリ秒、最大 ${Math.max(...times).toFixed(1)}ミリ秒（目標は100ミリ秒以内）`;
  info.annotations.push({ type: '応答時間', description: text });
  console.log(text);
  // 目標の100ミリ秒は中央値で確かめる。共有のCI機は遅いことがあるので、95%には余裕を見る。
  expect(median(times)).toBeLessThan(100);
  expect(p95(times)).toBeLessThan(250);
});
