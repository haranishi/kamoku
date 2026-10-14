// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 帳票（SC-06、AC-08.7）。6つのタブ、期間の変更（年・月・同じ年の中の自由な期間、貸借対照表は日付）、
// 選んだ期間がアドレスに入ること、各画面の「CSVで書き出す」のリンク（文字コードの選択つき）。

import { test, expect } from '@playwright/test';
import { open, withSettings } from './helpers.mjs';

const thisYear = new Date().getFullYear();
const TABS = [
  ['月次推移', 'monthly', 'income-statement.csv'],
  ['損益計算書', 'income-statement', 'income-statement.csv'],
  ['貸借対照表', 'balance-sheet', 'balance-sheet.csv'],
  ['試算表', 'trial-balance', 'trial-balance.csv'],
  ['総勘定元帳', 'ledger', 'ledger.csv'],
  ['消費税', 'tax-summary', 'tax-summary.csv'],
];
const exportLink = (page) => page.getByRole('link', { name: 'CSVで書き出す' });

test('AC-08.7 6つのタブを切り替えて表示でき、各画面にCSVで書き出すリンクがある', async ({ page }) => {
  await open(page, '#/reports');
  await expect(page).toHaveURL(/#\/reports\/monthly/);
  for (const [label, id, csv] of TABS) {
    await page.getByRole('tab', { name: label }).click();
    await expect(page).toHaveURL(new RegExp(`#/reports/${id}`));
    await expect(page.getByRole('tab', { name: label })).toHaveAttribute('aria-selected', 'true');
    await expect(page.locator('.report-title')).toContainText(label);
    if (id === 'ledger') {
      // 元帳は、科目を選ぶまで書き出せない。
      await expect(page.locator('.report-content')).toContainText('科目を選んでください');
      await expect(exportLink(page)).toHaveAttribute('aria-disabled', 'true');
      continue;
    }
    await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`/api/v1/export/${csv.replace('.', '\\.')}\\?.*encoding=utf8bom`));
    if (id === 'trial-balance') {
      // 試算表は、期首・借方・貸方・期末の4つの数字の列（設計レビューで加わった条件）。
      await expect(page.getByRole('table', { name: '試算表' }).locator('thead th')).toHaveText(['科目', '期首', '借方', '貸方', '期末']);
    }
  }
  await page.getByRole('tab', { name: '月次推移' }).click();
  await expect(page.locator('.report-content svg[role="img"]')).toHaveAttribute('aria-label', /推移/);
  await expect(page.getByRole('table', { name: /月次推移（グラフと同じ数字）/ })).toBeVisible();
});

test('AC-08.7 期間を年・月・自由な期間で変えられ、アドレスに入る。年をまたぐ期間は、選ぶ時点で止める', async ({ page }) => {
  await open(page, '#/reports/income-statement');
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年`);
  // 年: [ キー（前の年のボタンと同じ）と、選択欄。選択欄の年は開業日の年からなので、前の年へは [ で移る
  // （開業日が今年の帳簿でも動くように）。
  await page.locator('main h1').focus();
  await page.keyboard.press('[');
  await expect(page).toHaveURL(new RegExp(`year=${thisYear - 1}`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear - 1}年`);
  await page.getByRole('combobox', { name: '年', exact: true }).selectOption(String(thisYear));
  await expect(page).toHaveURL(new RegExp(`year=${thisYear}`));
  // 月。
  await page.locator('.seg-option', { hasText: '月' }).first().click();
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-\\d{2}`));
  await page.getByRole('combobox', { name: '月', exact: true }).selectOption('03');
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-03`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年3月`);
  // 自由な期間（同じ年の中）。
  await page.locator('.seg-option', { hasText: '自由な期間' }).click();
  const from = page.getByLabel('期間の始め');
  const to = page.getByLabel('期間の終わり');
  await from.fill('0401');
  await from.press('Tab');
  await to.fill('0630');
  await to.press('Tab');
  await page.getByRole('button', { name: 'この期間で見る' }).click();
  await expect(page).toHaveURL(new RegExp(`from=${thisYear}-04-01&to=${thisYear}-06-30`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}-04-01 〜 ${thisYear}-06-30`);
  // 年をまたぐ日付は受け付けず、理由を出して問い合わせない。
  await to.fill(`${thisYear + 1}-01-31`);
  await to.press('Tab');
  await expect(page.locator('.period-controls .field-error')).toContainText(`${thisYear}年の中の日付にしてください`);
  await expect(page.getByRole('button', { name: 'この期間で見る' })).toBeDisabled();
  await expect(page).toHaveURL(new RegExp(`to=${thisYear}-06-30`));
  // 選んだ期間は、CSVのリンクにも入る。
  await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`from=${thisYear}-04-01&to=${thisYear}-06-30`));
  // 戻る操作で、前の期間に戻る。
  await page.goBack();
  await expect(page).toHaveURL(new RegExp(`month=${thisYear}-03`));
  await expect(page.locator('.report-title')).toHaveText(`損益計算書　${thisYear}年3月`);
});

test('AC-08.7 貸借対照表は日付を1つ選ぶ。当期純利益の行があり、資産と負債・資本が一致する', async ({ page }) => {
  await open(page, '#/reports/balance-sheet');
  const date = page.getByLabel('いつの時点か');
  await date.fill(`${thisYear}0630`);
  await date.press('Enter');
  await date.press('Tab');
  await expect(page).toHaveURL(new RegExp(`as_of=${thisYear}-06-30`));
  await expect(page.locator('.report-title')).toHaveText(`貸借対照表　${thisYear}-06-30 時点`);
  await expect(page.locator('.report-content')).toContainText('当期純利益');
  await expect(page.locator('.report-check')).toContainText('一致しています');
  await expect(exportLink(page)).toHaveAttribute('href', new RegExp(`/export/balance-sheet\\.csv\\?as_of=${thisYear}-06-30`));
});

test('AC-08.6 の画面側: 文字コードを Shift_JIS にするとリンクが変わり、書き出したCSVを受け取れる（既定はBOMつきUTF-8）', async ({ page }) => {
  await open(page, '#/reports/trial-balance');
  const href = await exportLink(page).getAttribute('href');
  const res = await page.request.get(new URL(href, page.url()).href);
  expect(res.status()).toBe(200);
  expect(res.headers()['content-type']).toContain('text/csv');
  const bytes = await res.body();
  expect([...bytes.subarray(0, 3)]).toEqual([0xef, 0xbb, 0xbf]);
  await page.getByLabel('CSVの文字コード').selectOption('sjis');
  await expect(exportLink(page)).toHaveAttribute('href', /encoding=sjis/);
  const sj = await page.request.get(new URL(await exportLink(page).getAttribute('href'), page.url()).href);
  expect(sj.status()).toBe(200);
  expect([...(await sj.body()).subarray(0, 3)]).not.toEqual([0xef, 0xbb, 0xbf]);
  await page.getByLabel('CSVの文字コード').selectOption('utf8bom');
});

test('SC-06 総勘定元帳: 科目を選ぶと行と残高の推移が出て、行から仕訳の詳細に飛べる', async ({ page }) => {
  await open(page, '#/reports/ledger');
  const acc = page.getByRole('combobox', { name: '科目' });
  await acc.click();
  await page.keyboard.insertText('げんきん');
  await page.getByRole('option', { name: /現金/ }).last().click();
  await expect(page).toHaveURL(/account=/);
  const rows = page.locator('.ledger-table tbody tr');
  await expect(rows.first()).toBeVisible();
  await expect(page.locator('.ledger-facts')).toContainText('期首の残高');
  await expect(exportLink(page)).toHaveAttribute('href', /\/export\/ledger\.csv\?account=/);
  await rows.first().focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#\/journal\/e_/);
  await expect(page.getByRole('dialog')).toContainText('の仕訳');
});

test('SC-06 消費税: 上に「参考値」の注意と表の最終確認日。免税の設定では冒頭に案内を出す', async ({ page }) => {
  await open(page, '#/reports/tax-summary');
  const content = page.locator('.report-content');
  await expect(content).toContainText('参考値。簡易課税・特例は反映していません');
  await expect(content).toContainText(/経過措置の表の最終確認日は\d{4}-\d{2}-\d{2}です。/);
  await expect(content).toContainText('納付額の見込み');
  await withSettings(page, { tax_mode: 'exempt', invoice_number: null }, async () => {
    await page.reload();
    const first = page.locator('.report-content .banner').first();
    await expect(first).toContainText('設定上は免税事業者として表示しています。申告の要否は、国税庁の案内か税理士に確かめてください');
  });
});
