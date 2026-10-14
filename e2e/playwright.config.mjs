// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 画面の自動試験の設定。既定では、模擬サーバー（ui/dev/mock-server.mjs）を2つ立てる。
// - 8766: 操作の試験用（試験が仕訳を足すので、中身は変わっていく）
// - 8767: スクリーンショット用（今日を 2026-10-14 に固定し、何も書き込まない）
// - 8768: 初期設定の試験用（--fresh。初期設定の前の帳簿）
// - 8769: 初期設定のスクリーンショット用（--fresh。今日を 2026-10-14 に固定）
// 本物のサーバーで試すときは、KAMOKU_URL にそのアドレスを入れる（模擬サーバーは立てない）。
// 初期設定の試験は、別に立てた空の帳簿のサーバーを KAMOKU_SETUP_URL（合言葉は KAMOKU_SETUP_TOKEN）で渡す。
// ブラウザは新しく取得しない。~/Library/Caches/ms-playwright にある Chromium か、
// KAMOKU_E2E_CHANNEL=chrome で入っている Google Chrome を使う。

import { defineConfig } from '@playwright/test';

const external = process.env.KAMOKU_URL;
const flowsURL = external || 'http://127.0.0.1:8766';
const screensURL = process.env.KAMOKU_SCREENS_URL || external || 'http://127.0.0.1:8767';
const channel = process.env.KAMOKU_E2E_CHANNEL || undefined;

export default defineConfig({
  testDir: '.',
  testMatch: /.*\.spec\.mjs$/,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  // 試験どうしが同じ模擬サーバーの設定（課税の区分など）を変えるので、1つずつ順に動かす。
  workers: 1,
  fullyParallel: false,
  reporter: [['list']],
  outputDir: process.env.KAMOKU_E2E_OUT || './test-results',
  use: {
    channel,
    locale: 'ja-JP',
    timezoneId: 'Asia/Tokyo',
    viewport: { width: 1440, height: 900 },
    trace: 'retain-on-failure',
  },
  projects: [
    { name: 'flows', testIgnore: /screens\.spec\.mjs/, use: { baseURL: flowsURL } },
    { name: 'screens', testMatch: /screens\.spec\.mjs/, use: { baseURL: screensURL } },
  ],
  webServer: external
    ? undefined
    : [
        {
          command: 'node ../ui/dev/mock-server.mjs --port 8766 --quiet',
          url: 'http://127.0.0.1:8766/api/v1/status',
          reuseExistingServer: true,
          timeout: 15_000,
        },
        {
          command: 'node ../ui/dev/mock-server.mjs --port 8767 --quiet --today 2026-10-14',
          url: 'http://127.0.0.1:8767/api/v1/status',
          reuseExistingServer: true,
          timeout: 15_000,
        },
        {
          command: 'node ../ui/dev/mock-server.mjs --port 8768 --quiet --fresh',
          url: 'http://127.0.0.1:8768/api/v1/status',
          reuseExistingServer: true,
          timeout: 15_000,
        },
        {
          command: 'node ../ui/dev/mock-server.mjs --port 8769 --quiet --fresh --today 2026-10-14',
          url: 'http://127.0.0.1:8769/api/v1/status',
          reuseExistingServer: true,
          timeout: 15_000,
        },
      ],
});
