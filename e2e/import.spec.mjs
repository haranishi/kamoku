// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取込（SC-04、AC-04.3）。合成のCSV（crates/kamoku-import/tests/fixtures/synthetic/）で3段階を通す。
// 同じファイルをもう一度取り込むと重複と出て、行ごとに「この行も取り込む」を選べる。文字コードの選び直し、取引番号の列、期限切れ、壊れたCSV。
// 取込の重複は口座ごとなので、試験ごとに新しい口座を作る（前の実行の取込と混ざらないように）。

import { test, expect } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { open, uniqueText, newWallet, apiCall } from './helpers.mjs';

const FIXTURES = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', 'crates', 'kamoku-import', 'tests', 'fixtures', 'synthetic');
const fixture = (name) => path.join(FIXTURES, name);

async function start(page, file, type = 'bank') {
  await open(page, '#/');
  const w = await newWallet(page, type, uniqueText('E2E 取込口座'));
  await open(page, '#/import');
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await page.locator('input[type="file"]').setInputFiles(fixture(file));
  await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  return w;
}

test('AC-04.3 UTF-8 の明細（入金と出金が別の列）を3段階で取り込み、同じファイルをもう一度取り込むと重複と出る', async ({ page }) => {
  const w = await start(page, 'utf8.csv');
  const card = page.locator('.import-card');
  await expect(card).toContainText('UTF-8（自動で判定）');
  await expect(card).toContainText('見出しの言葉から、列の対応を推測しました');
  await expect(page.getByLabel('日付の列')).toHaveValue('0');
  await expect(page.getByRole('radio', { name: '入金と出金が別の列' })).toBeChecked();
  await expect(page.getByLabel('出金の列')).toHaveValue('2');
  await expect(page.getByLabel('入金の列')).toHaveValue('3');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.getByRole('heading', { name: '3. 確かめて取り込む' })).toBeVisible();
  await expect(page.locator('.check-counts')).toContainText('新規4件');
  await expect(page.locator('.check-counts')).toContainText('重複0件');
  await page.getByRole('button', { name: '4件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('4件を未確認に入れました');
  await page.locator('.import-done').getByRole('link', { name: /未確認を開く/ }).click();
  await expect(page).toHaveURL(new RegExp(`#/inbox\\?wallet=${w.code}`));
  await expect(page.locator('tr.inbox-row')).toHaveCount(4);

  // 同じファイルをもう一度。口座の列の対応を覚えているので、その旨が出る。全部が重複になる。
  await page.goto(`/#/import?wallet=${w.code}`);
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await expect(page.locator('.import-card')).toContainText('この口座で前に使った列の対応を入れました');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規0件');
  await expect(page.locator('.check-counts')).toContainText('重複4件');
  await expect(page.locator('.check-table')).toContainText('すでに取り込んであります');
  const commit = page.getByRole('button', { name: /件を取り込む/ });
  await expect(commit).toBeDisabled();
  // 重複の行も、選べば取り込める（既定は取り込まない）。
  await page.getByRole('checkbox', { name: /^5行目も取り込む/ }).check();
  await expect(commit).toHaveText(/1件を取り込む/);
  await commit.click();
  await expect(page.locator('.import-done')).toContainText('1件を未確認に入れました');
  await expect(page.locator('.import-done')).toContainText('重複 3件');
  const drafts = (await apiCall(page, 'GET', `/drafts?wallet=${w.code}&limit=50`)).json;
  expect(drafts.total).toBe(5);
});

test('AC-04.3 Shift_JIS の明細（元号の日付・機種依存文字）を読み、文字コードを選び直すと読み直す', async ({ page }) => {
  await start(page, 'sjis.csv');
  const card = page.locator('.import-card');
  await expect(card).toContainText('Shift_JIS（自動で判定）');
  await expect(page.locator('.sample-table')).toContainText('デモ商店①');
  await expect(page.locator('.sample-table')).toContainText('令和8年10月2日');
  // UTF-8 を選ぶと、読めない文字があると出る（選び直しが効いている）。
  await page.getByLabel('文字コード').selectOption('utf-8');
  await expect(card.locator('.banner')).toContainText('読めない文字がありました');
  await page.getByLabel('文字コード').selectOption('shift_jis');
  await expect(card.locator('.banner')).toHaveCount(0);
  await expect(page.locator('.sample-table')).toContainText('デモ商店①');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規3件');
  const table = page.locator('.check-table');
  await expect(table).toContainText('2026-10-01');
  await expect(table).toContainText('2026-10-02');
  await expect(table).toContainText('+300,000');
});

test('AC-04.3 符号つきの1列は、入金の向きを直せる（正の数が入金）', async ({ page }) => {
  await start(page, 'signed.csv');
  await expect(page.getByRole('radio', { name: '符号つきの1列' })).toBeChecked();
  await expect(page.getByLabel('金額の列')).toHaveValue('2');
  // 推測はカードの明細の向き（正の数が出金）。銀行の明細なので直す。
  await expect(page.getByRole('radio', { name: /正の数が出金/ })).toBeChecked();
  // 切り替えの選択肢は、見た目のラベルを押して選ぶ（中の入力欄は見えない）。
  await page.locator('.seg-option', { hasText: '正の数が入金' }).click();
  await expect(page.getByRole('radio', { name: /正の数が入金/ })).toBeChecked();
  await page.getByRole('button', { name: '確かめる' }).click();
  const rows = page.locator('.check-table tbody tr');
  await expect(rows.filter({ hasText: 'サンプル給与' })).toContainText('+300,000');
  await expect(rows.filter({ hasText: '手数料 デモ' })).toContainText('-220');
});

test('SC-04 取引番号の列（任意）を選ぶと、摘要が変わっていても同じ番号の行を重複と見分ける', async ({ page }) => {
  // 番号の列がある明細を、試験の中で作る（合成の固定ファイルには番号の列が無いため）。
  const csv = (desc) => ({ name: 'with-ref.csv', mimeType: 'text/csv', buffer: Buffer.from(`日付,摘要,出金,入金,取引番号\n2026/10/01,${desc} 1,1650,,A-0001\n2026/10/02,${desc} 2,420,,A-0002\n`) });
  await open(page, '#/');
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込番号'));
  const load = async (desc) => {
    // 取り込んだ後もアドレスが同じなので、読み込み直して最初の段階から始める（# だけの移動では画面が変わらない）。
    await page.goto(`/#/import?wallet=${w.code}`);
    await page.reload();
    await page.locator('input[type="file"]').setInputFiles(csv(desc));
    await expect(page.getByRole('heading', { name: '2. 列の対応' })).toBeVisible();
  };
  await load('サンプル店');
  // 番号の列は推測しない（既定は「使わない」）。
  const idSel = page.getByLabel('取引番号の列（任意）');
  await expect(idSel).toHaveValue('');
  await idSel.selectOption('4');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規2件');
  await page.getByRole('button', { name: '2件を取り込む' }).click();
  await expect(page.locator('.import-done')).toContainText('2件を未確認に入れました');
  // 摘要だけ変えた同じ番号の明細。保存した対応（番号の列）で、2行とも重複になる。
  await load('デモ店');
  await expect(idSel).toHaveValue('4');
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.locator('.check-counts')).toContainText('新規0件');
  await expect(page.locator('.check-counts')).toContainText('重複2件');
});

test('SC-04 口座が選ばれるまではファイルを置けない。壊れたCSVは行の番号つきで断る', async ({ page }) => {
  await open(page, '#/import');
  const pick = page.getByRole('button', { name: 'ファイルを選ぶ' });
  await expect(pick).toBeDisabled();
  await expect(page.locator('.drop-zone')).toContainText('先に、上で口座を選んでください');
  // 口座を選ばずに置こうとすると、理由を出して受け取らない。
  await page.locator('input[type="file"]').setInputFiles(fixture('utf8.csv'));
  await expect(page.locator('.import-step .field-error').last()).toContainText('先に口座を選んでください');
  await expect(page.getByRole('heading', { name: '1. 口座とファイル' })).toBeVisible();
  const w = await newWallet(page, 'bank', uniqueText('E2E 取込壊れ'));
  await page.reload();
  await page.getByLabel('取り込む口座').selectOption(w.code);
  await expect(pick).toBeEnabled();
  const failed = page.waitForResponse((r) => r.url().includes('/api/v1/imports/upload'));
  await page.locator('input[type="file"]').setInputFiles(fixture('broken_quote.csv'));
  // サーバーは invalid_csv と行の番号（line）で断る。文面は模擬サーバーと本物で違うので、画面は行の番号が出ることを見る。
  const res = await failed;
  const body = await res.json();
  expect([res.status(), body.error.code, body.error.line]).toEqual([400, 'invalid_csv', 3]);
  await expect(page.locator('.import-step .field-error').last()).toContainText('3行目');
});

test('SC-04 期限切れ（404）は「もう一度ファイルを選んでください」と出して、最初の段階に戻る', async ({ page }) => {
  await start(page, 'utf8.csv');
  await page.route('**/api/v1/imports/preview', (route) => route.fulfill({ status: 404, json: { error: { code: 'not_found', message: 'not found' } } }));
  await page.getByRole('button', { name: '確かめる' }).click();
  await expect(page.getByRole('heading', { name: '1. 口座とファイル' })).toBeVisible();
  await expect(page.locator('.import-step')).toContainText('もう一度ファイルを選んでください');
});
