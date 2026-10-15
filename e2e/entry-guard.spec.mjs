// SPDX-FileCopyrightText: 2026 Ikki Nishihara and the kamoku contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

// 取引を登録・訂正（SC-02）の、離れる前の確認・開いたときのフォーカス・日付の取り違えの確認。
// 入力がある間は、どの道で離れても（サイドのリンク・戻る・g・⌘K・タブを閉じる）、Esc と同じ確認を出す。
// 保存して離れる・やめるで離れるときは、確認を出さない。

import { test, expect } from '@playwright/test';
import { open, apiCall, uniqueText, entryFields, addCashExpense, walletOf } from './helpers.mjs';

const discard = (page) => page.getByRole('dialog', { name: '入力中の内容を捨てますか' });

/** 金額と科目（消耗品費）を入れる。 */
async function fillSome(page, amount = '1100') {
  const f = entryFields(page);
  await f.amount.fill(amount);
  await f.account.click();
  await page.keyboard.insertText('消耗品費');
  await page.keyboard.press('Enter');
  return f;
}

test('SC-02 入力があると、サイドのリンクで離れる前に確認が出る。「入力を続ける」で残り、「捨てて離れる」で行き先へ進む', async ({ page }) => {
  await open(page, '#/entry');
  const f = await fillSome(page);
  const side = page.locator('.sidebar');
  await side.getByRole('link', { name: '仕訳帳' }).click();
  const dialog = discard(page);
  await expect(dialog).toBeVisible();
  await expect(page).toHaveURL(/#\/entry$/);
  // 既定は「入力を続ける」（Enterで続ける）。説明の文には id があり、小窓に結ぶ（describedBy）。
  await expect(dialog.getByRole('button', { name: '入力を続ける' })).toBeFocused();
  await expect(dialog.locator('p[id]')).toHaveText('まだ保存していない入力があります。捨てると、元に戻せません。');
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(page).toHaveURL(/#\/entry$/);
  await expect(f.amount).toHaveValue('1,100');
  await expect(f.account).toHaveValue('消耗品費');
  // 最後にいた欄（科目の次の口座）へ戻る。押したリンクには残らない。
  await expect(f.wallet).toBeFocused();
  await side.getByRole('link', { name: '仕訳帳' }).click();
  await discard(page).getByRole('button', { name: '捨てて離れる' }).click();
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
});

test('SC-02 入力が無ければ（打って消したときも）、確認なしで離れる', async ({ page }) => {
  await open(page, '#/entry');
  const f = entryFields(page);
  await f.amount.fill('500');
  await f.amount.fill('');
  await f.description.fill('一度打った摘要');
  await f.description.fill('');
  await page.locator('.sidebar').getByRole('link', { name: '仕訳帳' }).click();
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(discard(page)).toHaveCount(0);
});

test('SC-02 戻る・g のあとの文字・⌘K で離れるときも確認が出る。戻るで残っても、履歴の並びは元のまま', async ({ page }) => {
  await open(page, '#/');
  await page.keyboard.press('n');
  const f = entryFields(page);
  await expect(f.date).toBeFocused();
  await f.amount.fill('2200');
  const stay = async () => {
    await expect(discard(page)).toBeVisible();
    await expect(page).toHaveURL(/#\/entry$/);
    await discard(page).getByRole('button', { name: '入力を続ける' }).click();
    await expect(discard(page)).toHaveCount(0);
    await expect(page).toHaveURL(/#\/entry$/);
    // 小窓が閉じ終わると、フォーカスは最後にいた欄（金額）へ戻る。戻るまで待ってから次の操作をする
    // （閉じる処理より先に見出しへ移すと、あとから金額の欄へ戻り、続けて押した g j が文字として入る）。
    await expect(f.amount).toBeFocused();
  };
  await page.goBack();
  await stay();
  await page.locator('main h1').focus();
  await page.keyboard.press('g');
  await page.keyboard.press('j');
  await stay();
  await page.keyboard.press('ControlOrMeta+k');
  await page.keyboard.insertText('帳票');
  await page.keyboard.press('Enter');
  await stay();
  await expect(f.amount).toHaveValue('2,200');
  // もう一度戻ると、来たホームへ戻れる（確認の間に、履歴の行を書き換えていない）。
  await page.goBack();
  await discard(page).getByRole('button', { name: '捨てて離れる' }).click();
  await expect(page).toHaveURL(/#\/$/);
  await expect(page.locator('main h1')).toHaveText('ホーム');
});

test('SC-02 Esc の「捨てて戻る」は、来た画面へ戻る（ホームに固定しない）', async ({ page }) => {
  await open(page, '#/journal');
  await page.locator('main h1').focus();
  await page.keyboard.press('n');
  const f = entryFields(page);
  await expect(f.date).toBeFocused();
  await f.amount.fill('3300');
  await page.keyboard.press('Escape');
  const dialog = discard(page);
  await expect(dialog.getByRole('button', { name: '入力を続ける' })).toBeFocused();
  await dialog.getByRole('button', { name: '捨てて戻る' }).click();
  await expect(page).toHaveURL(/#\/journal$/);
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
});

test('SC-02 アドレスを直に開いた取引を登録では、Esc の「捨てて戻る」でホームへ', async ({ page }) => {
  // 合言葉で入ったタブとは別の、新しいタブで開く（そのタブの履歴に、アプリの前の画面が無い）。
  await open(page, '#/');
  const tab = await page.context().newPage();
  await tab.goto('/#/entry');
  const f = entryFields(tab);
  await expect(f.date).toBeFocused();
  await f.amount.fill('4400');
  await tab.keyboard.press('Escape');
  await discard(tab).getByRole('button', { name: '捨てて戻る' }).click();
  await expect(tab).toHaveURL(/#\/$/);
  await expect(tab.locator('main h1')).toHaveText('ホーム');
  await tab.close();
});

test('SC-02 保存して続けた後と、「保存」で仕訳へ移るときは、離れる確認を出さない', async ({ page }) => {
  await open(page, '#/entry');
  let f = await fillSome(page);
  // 口座は、保存して続けた後も残る欄。開いたときと違う口座にしておく（残った値を「入力」と取り違えないことを確かめる）。
  const bank = await walletOf(page, 'bank');
  await f.wallet.click();
  await page.keyboard.insertText(bank.name);
  await page.keyboard.press('Enter');
  await expect(f.wallet).toHaveValue(bank.name);
  await f.description.fill(uniqueText('E2E 離れる確認なし'));
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  await expect(page.locator('.toast').filter({ hasText: 'を登録しました' })).toBeVisible();
  await expect(f.amount).toBeFocused();
  await page.locator('.sidebar').getByRole('link', { name: '仕訳帳' }).click();
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
  await expect(discard(page)).toHaveCount(0);
  await page.locator('.sidebar').getByRole('link', { name: '取引を登録' }).click();
  f = await fillSome(page, '1200');
  await f.description.fill(uniqueText('E2E 保存で移る'));
  await page.getByRole('button', { name: '保存', exact: true }).click();
  await expect(page).toHaveURL(/#\/journal\/e_[0-9a-f]+$/);
  await expect(discard(page)).toHaveCount(0);
});

test('SC-02 入力があるときに読み込み直すと、ブラウザの標準の確認が出る。入力が無ければ出ない', async ({ page }) => {
  await open(page, '#/entry');
  const kinds = [];
  page.on('dialog', (d) => {
    kinds.push(d.type());
    d.accept();
  });
  await page.reload();
  await expect(page.locator('main h1')).toBeVisible();
  expect(kinds).toEqual([]);
  await entryFields(page).amount.fill('100');
  await page.reload();
  await expect(page.locator('main h1')).toBeVisible();
  expect(kinds).toEqual(['beforeunload']);
});

test('SC-02 開いたとき、新規は日付、訂正は金額の欄にフォーカスがある。フォームの名前は見出し', async ({ page }) => {
  await open(page, '#/');
  await page.locator('.sidebar').getByRole('link', { name: '取引を登録' }).click();
  const f = entryFields(page);
  await expect(f.date).toBeFocused();
  await expect(page.getByRole('form', { name: '取引を登録' })).toBeVisible();
  const made = await addCashExpense(page, { description: uniqueText('E2E 訂正のフォーカス') });
  await open(page, `#/journal/${made.id}`);
  await expect(page.getByRole('dialog')).toContainText('訂正する');
  await page.keyboard.press('e');
  await expect(page).toHaveURL(new RegExp(`#/entry/${made.id}$`));
  await expect(f.amount).toBeFocused();
  await expect(page.getByRole('form', { name: '取引を訂正' })).toBeVisible();
});

test('SC-02 今日より31日を超えて先の日付は、保存の前に1回だけ確かめる。「直す」が既定で、承認すると保存する', async ({ page }) => {
  await open(page, '#/entry');
  const f = await fillSome(page);
  await f.date.fill('20300101');
  await f.date.press('Tab');
  // 保存の要求は、画面の中で受け止めて帳簿に書かない（2030年の仕訳を、ほかの試験の一覧の先頭に混ぜないため）。
  const posted = [];
  await page.route('**/api/v1/entries', async (route) => {
    if (route.request().method() !== 'POST') return route.continue();
    posted.push(route.request().postDataJSON());
    return route.fulfill({ status: 201, json: { id: 'e_00000000000000000000', no: 9999 } });
  });
  const save = page.getByRole('button', { name: /保存して続ける/ });
  await save.click();
  const dialog = page.getByRole('dialog', { name: '日付は合っていますか' });
  await expect(dialog).toContainText('2030年1月1日の取引として保存します。');
  await expect(dialog).toContainText('今日より31日を超えて先の日付です。年は合っていますか。');
  await expect(dialog.getByRole('button', { name: '直す' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(dialog).toHaveCount(0);
  await expect(f.date).toBeFocused();
  expect(posted).toHaveLength(0);
  await save.click();
  await dialog.getByRole('button', { name: 'このまま保存' }).click();
  await expect(page.locator('.toast').filter({ hasText: 'No.9999 を登録しました' })).toBeVisible();
  expect(posted).toHaveLength(1);
  expect(posted[0].date).toBe('2030-01-01');
});

test('SC-02 開業日より前の日付も、保存の前に確かめる', async ({ page }) => {
  const opened = (await apiCall(page, 'GET', '/settings')).json.opened_on;
  test.skip(!/^\d{4}-\d{2}-\d{2}$/.test(opened || ''), '開業日の無い帳簿');
  const [y, m, d] = opened.split('-').map(Number);
  const before = new Date(Date.UTC(y, m - 1, d - 1)).toISOString().slice(0, 10);
  await open(page, '#/entry');
  const f = await fillSome(page);
  await f.date.fill(before.replaceAll('-', ''));
  await f.date.press('Tab');
  await page.getByRole('button', { name: /保存して続ける/ }).click();
  const dialog = page.getByRole('dialog', { name: '日付は合っていますか' });
  await expect(dialog).toContainText(`開業日（${y}年${m}月${d}日）より前の日付です。`);
  await dialog.getByRole('button', { name: '直す' }).click();
  await expect(f.date).toBeFocused();
});

test('SC-02 幅390: 下のタブで離れるときも確認が出る', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await open(page, '#/entry');
  await entryFields(page).amount.fill('5500');
  await page.locator('.bottomnav').getByRole('link', { name: '仕訳帳' }).click();
  await expect(discard(page)).toBeVisible();
  await expect(page).toHaveURL(/#\/entry$/);
  await discard(page).getByRole('button', { name: '捨てて離れる' }).click();
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
});

test('SC-02 仕訳で入力に切り替えて入れた行も、離れる前に確かめる', async ({ page }) => {
  await open(page, '#/entry');
  await page.getByRole('tab', { name: '仕訳で入力' }).click();
  const rows = page.locator('form.entry-form .jl-table tbody tr');
  await rows.nth(0).getByLabel('メモ').fill('仕訳の形のメモ');
  await page.locator('.sidebar').getByRole('link', { name: '仕訳帳' }).click();
  await expect(discard(page)).toBeVisible();
  await discard(page).getByRole('button', { name: '入力を続ける' }).click();
  await expect(rows.nth(0).getByLabel('メモ')).toHaveValue('仕訳の形のメモ');
  await expect(rows.nth(0).getByLabel('メモ')).toBeFocused();
});

test('SC-02 訂正: 開いただけなら確認なしで離れる。訂正の理由だけでも、入れたら確かめる', async ({ page }) => {
  const made = await addCashExpense(page, { description: uniqueText('E2E 訂正の離れる確認') });
  await open(page, `#/entry/${made.id}`);
  const f = entryFields(page);
  await expect(f.amount).toBeFocused();
  await page.locator('.sidebar').getByRole('link', { name: '仕訳帳' }).click();
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
  await expect(discard(page)).toHaveCount(0);
  await open(page, `#/entry/${made.id}`);
  await page.getByLabel('訂正の理由').fill('理由だけ書いた');
  await page.locator('.sidebar').getByRole('link', { name: '仕訳帳' }).click();
  await expect(discard(page)).toBeVisible();
  await discard(page).getByRole('button', { name: '捨てて離れる' }).click();
  await expect(page.locator('main h1')).toHaveText('仕訳帳');
});

// Esc の確認（捨てて戻る）が開いたまま、ブラウザの「戻る」を押すと、2枚目が重なり、1枚目は移動した後も残った
// （2026-10-15 の体験の評価 E4）。確認は1枚だけ。新しく開くときは、前のを「続ける」側で閉じる。
test('SC-02 「入力中の内容を捨てますか」は1枚だけ。Esc の確認が開いたままブラウザの「戻る」を押しても、重ならず、移動した後に残らない', async ({ page }) => {
  await open(page, '#/');
  await page.evaluate(() => {
    location.hash = '#/entry';
  });
  await fillSome(page);
  await page.keyboard.press('Escape');
  await expect(discard(page)).toHaveCount(1);
  await expect(discard(page).getByRole('button', { name: '捨てて戻る' })).toBeVisible();
  await page.goBack();
  // 「戻る」で、移動の確認が出る。重ならず、1枚だけ（Esc の確認は閉じて、入力を続ける側として扱う）。
  await expect(discard(page).getByRole('button', { name: '捨てて離れる' })).toBeVisible();
  await expect(discard(page)).toHaveCount(1);
  await discard(page).getByRole('button', { name: '捨てて離れる' }).click();
  await expect(page).toHaveURL(/#\/$/);
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
