// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { expect } from '@playwright/test';

/** Fresh HTTP caches; browser process, OS and synthetic server stay warm. */
export async function readinessSamples(browser, authenticate) {
  const samples = { baseline: [], rust: [] };
  for (let i = 0; i < 5; i++) {
    for (const variant of (i % 2 ? ['rust', 'baseline'] : ['baseline', 'rust'])) {
      const context = await browser.newContext({
        baseURL: process.env.KAMOKU_URL, locale: 'ja-JP', timezoneId: 'Asia/Tokyo',
        viewport: { width: 1440, height: 900 },
      });
      try {
        const page = await context.newPage();
        await authenticate(page);
        const response = await page.goto(variant === 'rust' ? '/rust/index.html' : '/#/journal');
        expect(response.status()).toBe(200);
        const selector = variant === 'rust' ? '.table-card[aria-busy="false"] tbody tr' : '.journal-table tbody tr';
        await expect(page.locator(selector).first()).toBeVisible();
        samples[variant].push(await page.evaluate(() => performance.now()));
      } finally {
        await context.close();
      }
    }
  }
  const summarize = (values) => {
    const sorted = [...values].sort((a, b) => a - b);
    return { samples: values, median: sorted[2], p95: sorted[4] };
  };
  return { baseline: summarize(samples.baseline), rust: summarize(samples.rust) };
}
