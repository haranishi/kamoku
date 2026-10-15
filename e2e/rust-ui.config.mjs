// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

import { defineConfig } from '@playwright/test';

if (!process.env.KAMOKU_URL || !process.env.KAMOKU_TOKEN) {
  throw new Error('Run this suite with e2e/run-real.sh flows --config rust-ui.config.mjs after building the Rust UI.');
}

export default defineConfig({
  testDir: '.',
  testMatch: 'rust-preview.mjs',
  workers: 1,
  fullyParallel: false,
  timeout: 60_000,
  expect: { timeout: 8_000 },
  reporter: [['list']],
  outputDir: './rust-ui-results/playwright',
  use: {
    baseURL: process.env.KAMOKU_URL,
    locale: 'ja-JP',
    timezoneId: 'Asia/Tokyo',
    viewport: { width: 1440, height: 900 },
    // Auth/session values must never be uploaded inside traces or recordings.
    trace: 'off',
    video: 'off',
    screenshot: 'off',
  },
  projects: [{ name: 'flows' }],
});
