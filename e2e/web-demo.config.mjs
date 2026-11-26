// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// ブラウザのお試し版だけを、サブパス /kamoku-demo/ で配って試す。模擬サーバーは立てない。

import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from '@playwright/test';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = process.env.KAMOKU_WEB_DEMO_ROOT || path.resolve(here, '../target/web-demo');
const port = 8770;
const channel = process.env.KAMOKU_E2E_CHANNEL || undefined;

function quote(value) {
  return `'${String(value).replaceAll("'", `'\\''`)}'`;
}

export default defineConfig({
  testDir: here,
  testMatch: /web-demo\.spec\.mjs$/,
  timeout: 60_000,
  expect: { timeout: 20_000 },
  workers: 1,
  fullyParallel: false,
  reporter: [['list']],
  outputDir: path.join(here, 'test-results/web-demo'),
  use: {
    baseURL: `http://127.0.0.1:${port}/kamoku-demo/`,
    channel,
    locale: 'ja-JP',
    timezoneId: 'Asia/Tokyo',
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
    acceptDownloads: true,
  },
  webServer: {
    command: `python3 ${quote(path.join(here, 'web-demo-server.py'))} ${quote(root)} ${port}`,
    cwd: here,
    url: `http://127.0.0.1:${port}/kamoku-demo/`,
    reuseExistingServer: !process.env.CI,
    timeout: 15_000,
  },
});
