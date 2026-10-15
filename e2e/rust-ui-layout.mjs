// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { expect } from '@playwright/test';

/** Check actual glyph fragments, since body scrollWidth misses text overflowing a cell. */
export async function expectJournalTextFits(page, width) {
  const problems = await page.evaluate(() => {
    const problems = [];
    const card = document.querySelector('.table-card').getBoundingClientRect();
    const inspect = (element, limit, name, ancestor) => {
      const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT);
      let visibleFragments = 0;
      while (walker.nextNode()) {
        if (!walker.currentNode.textContent.trim()) continue;
        const range = document.createRange();
        range.selectNodeContents(walker.currentNode);
        for (const fragment of range.getClientRects()) {
          if (!fragment.width || !fragment.height) continue;
          visibleFragments++;
          for (const [bounds, label] of [[limit, name], ...(ancestor ? [[ancestor, 'table card']] : [])]) {
            if (fragment.left < bounds.left - 1 || fragment.right > bounds.right + 1 || fragment.top < bounds.top - 1 || fragment.bottom > bounds.bottom + 1) {
              problems.push({ label, text: walker.currentNode.textContent, fragment: fragment.toJSON(), bounds: bounds.toJSON() });
            }
          }
        }
      }
      if (element.textContent.trim() && visibleFragments === 0) problems.push({ label: `${name} has no visible text` });
    };
    document.querySelectorAll('tbody td').forEach((cell) => inspect(cell, cell.getBoundingClientRect(), cell.className, card));
    document.querySelectorAll('tbody .amount').forEach((amount) => inspect(amount, amount.closest('td').getBoundingClientRect(), 'amount', card));
    const caption = document.querySelector('caption');
    const captionBox = caption.getBoundingClientRect();
    const captionStyle = getComputedStyle(caption);
    const maxCaptionHeight = 3 * parseFloat(captionStyle.lineHeight) + parseFloat(captionStyle.paddingTop) + parseFloat(captionStyle.paddingBottom);
    inspect(caption, captionBox, 'caption', card);
    if (captionBox.width < card.width - 3) problems.push({ label: 'caption does not span the card', width: captionBox.width, cardWidth: card.width });
    if (!Number.isFinite(maxCaptionHeight) || captionBox.height > maxCaptionHeight + 1) problems.push({ label: 'caption exceeds three normal lines', height: captionBox.height, maxCaptionHeight });
    if (captionBox.bottom > document.querySelector('tbody').getBoundingClientRect().top + 1) problems.push({ label: 'caption overlaps rows' });
    const summary = document.querySelector('.summary');
    const summaryBox = summary.getBoundingClientRect();
    inspect(summary, summaryBox, 'summary');
    if (summaryBox.bottom > document.querySelector('.pager').getBoundingClientRect().top + 1) problems.push({ label: 'summary overlaps pager' });
    return problems;
  });
  expect(problems, `Text must fit its cell and summary at ${width}px`).toEqual([]);
  await expect(page.locator('caption')).toBeVisible();
  await expect(page.getByRole('table', { name: /日付の新しい順。詳細は通常の画面で開き/ })).toBeVisible();
  if (width < 1024) {
    const box = await page.getByRole('link', { name: '通常の仕訳帳へ' }).boundingBox();
    expect(box.width).toBeGreaterThanOrEqual(44);
    expect(box.height).toBeGreaterThanOrEqual(44);
  }
}
